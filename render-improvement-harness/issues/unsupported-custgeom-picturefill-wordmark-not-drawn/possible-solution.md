# Possible solution: unsupported-custgeom-picturefill-wordmark-not-drawn

## Approach

Two changes must land together; either alone leaves `typography-trick` wrong.

**A. The custGeom outline** — exactly the port described in
`render-improvement-harness/issues/geometry-custom-collapses-to-bbox/possible-solution.md`.
Nothing to add here except one constraint that cluster's corpus scan did not surface: this shape is
a *single* `a:path` holding 15 subpaths, so the walker must keep emitting after a `close` rather
than stopping at the first contour, and must not reorder or reverse contours — the counters of C, R
and A are punched out by `FillRule::Winding` (`crates/pptx-raster/src/lib.rs:382`) and by canvas's
default nonzero rule (`packages/pptx/src/render/canvas.ts:126-152`).

**B. A picture fill on a `p:sp`** — new capability, six crates plus the web package. Follow the
docx implementation, which already does all of this end to end:

1. **`crates/pptx-parse`** — give `parse_shape` (`crates/pptx-parse/src/drawing.rs:138`) the
   `relationships` slice its call site already holds
   (`crates/pptx-parse/src/drawing.rs:110`), and replace the bare
   `ShapeFill::named("picture")` at `crates/pptx-parse/src/drawing.rs:579-581` with a parse that
   captures `a:blip/@r:embed`, resolves it through `relationship_target`
   (`crates/pptx-parse/src/drawing.rs:930`), and records `a:srcRect` plus the
   `a:stretch/a:fillRect` (or `a:tile`) mode. `crates/docx-parse/src/shape.rs:620-636` is the exact
   shape of this code, including the `r:embed` / bare-`embed` fallback and
   `parse_picture_fill_mode` (`crates/docx-parse/src/shape.rs:651`).
   `parse_background` (`crates/pptx-parse/src/drawing.rs:554`) shares `parse_fill`, so a
   picture slide background comes along for free once the relationships reach it.

2. **Where the payload lives.** `ShapeFill` (`crates/ooxml-drawingml/src/shape.rs:7-14`) is shared
   with docx (`crates/docx-parse/src/text_box.rs:29`, `crates/docx-parse/src/drawingml.rs:123`), so
   widening it touches both formats' serialised models. Two options; pick one and say so in the PR:
   - Add `picture: Option<PictureFill>` to `ShapeFill` with
     `#[serde(default, skip_serializing_if = "Option::is_none")]` — one type, docx can adopt it
     later.
   - Or keep `ShapeFill` untouched and hang `picture_fill: Option<PictureFill>` off
     `pptx_parse::Shape` (`crates/pptx-parse/src/model.rs:198-207`) beside the geometry path the
     other cluster adds. Cheaper blast radius, one more field to thread.
   `PictureFill` needs `media_part_path`, `relationship_id`, `src_rect` and the stretch rect —
   `crates/docx-parse/src/shape.rs:36-70` (`ShapeFillPaint`) is the reference field set.

3. **`crates/pptx-edit`** — in the `ShapeNode::Shape` arm of `seed_shape`
   (`crates/pptx-edit/src/deck.rs:123-137`) write `mediaPartPath` when the fill is a picture, the
   way the `ShapeNode::Picture` arm already does at `crates/pptx-edit/src/deck.rs:144-146`.
   `ShapeSnapshot.media_part_path` (`crates/pptx-edit/src/model.rs:118`) and the read-back
   (`crates/pptx-edit/src/deck.rs:823`) are kind-agnostic already, so **this needs no
   `SCHEMA_VERSION` bump on its own**; if it ships with the geometry cluster's `geometryPathJson`,
   both ride that one bump.

4. **`crates/pptx-render`** — add a picture arm to `Paint`
   (`crates/pptx-render/src/display_list.rs:24-34`) carrying the asset id and the placement rects,
   and emit it from `paint` (`crates/pptx-render/src/layout.rs:1897`). `paint` currently takes only
   `(&ShapeFill, &Theme)`; it needs the snapshot's / node's media path too, so give it a third
   argument at all three call sites (`crates/pptx-render/src/layout.rs:388`, `:526` and, for a
   picture slide background, `:183`). The
   composed path (`crates/pptx-render/src/lib.rs:161-190`) deserialises `fill: Option<Paint>`
   straight from JSON and needs no code change.

5. **`crates/pptx-raster`** — in `paint_shape` (`crates/pptx-raster/src/lib.rs:364`), branch before
   `shader_paint`: for a picture paint, build a `Mask` from the already-built `Path` the way
   `clipped` does (`crates/pptx-raster/src/lib.rs:346-359`), intersect it with the incoming `clip`,
   then reuse the decode-and-fit block from `paint_image`
   (`crates/pptx-raster/src/lib.rs:401-425`) with the frame widened by the stretch rect. A
   `tiny_skia::Pattern` shader inside `shader_paint` is the tidier alternative but fights the
   `Paint<'static>` return type, since the pattern borrows the decoded pixmap. Count an
   undecodable blip in `skipped_images` (`crates/pptx-raster/src/lib.rs:426`) rather than dropping
   the shape silently — that is the bug being fixed.

6. **`packages/pptx`** — extend the `Paint` union (`packages/pptx/src/types.ts:205-212`) and teach
   `paintShape` (`packages/pptx/src/render/canvas.ts:117`) to clip to the built path and
   `drawImage` through the resolver. `paintShape` is currently sync while `resolveImage` is async
   (`packages/pptx/src/render/canvas.ts:15-17`), so `paintShape` becomes `async` and its call site
   (`packages/pptx/src/render/canvas.ts:71-72`) gains an `await`. `drawPictureShapeFill`
   (`packages/docx/src/layout/render/canvasBackend.ts:704-735`) is the working version of exactly
   this, including the fillRect band arithmetic and the fall-back-to-solid path when the resolver
   returns nothing.

## Sketch

```rust
// crates/pptx-parse/src/drawing.rs
fn parse_fill(element: &XmlElement, relationships: &[Relationship]) -> Option<ShapeFill> {
    // ... noFill / solidFill / gradFill unchanged ...
    if let Some(blip_fill) = element.child("blipFill") {
        let relationship_id = blip_fill
            .child("blip")
            .and_then(|b| b.attribute("r:embed").or_else(|| b.attribute_local("embed")))
            .map(str::to_owned);
        return Some(ShapeFill {
            fill_type: "picture".to_owned(),
            picture: Some(PictureFill {
                media_part_path: relationship_id
                    .as_deref()
                    .and_then(|id| relationship_target(relationships, id)),
                relationship_id,
                src_rect: parse_crop(blip_fill.child("srcRect")),
                // <a:stretch><a:fillRect l="-53000"/> -> the target band, per-mille of the box
                stretch: blip_fill
                    .child("stretch")
                    .and_then(|s| s.child("fillRect"))
                    .map(parse_relative_rect),
            }),
            ..ShapeFill::named("picture")
        });
    }
    None
}
```

```rust
// crates/pptx-raster/src/lib.rs, inside paint_shape, before shader_paint
if let Some(SlidePaint::Picture { asset_id, stretch, .. }) = fill {
    let mut mask = match clip {
        Some(existing) => existing.clone(),
        None => Mask::new(self.pixmap.width(), self.pixmap.height())
            .ok_or("invalid clip mask size")?,
    };
    // `path` already carries the primitive transform, so the mask takes identity.
    match clip {
        Some(_) => mask.intersect_path(&path, FillRule::Winding, true, Transform::identity()),
        None => mask.fill_path(&path, FillRule::Winding, true, Transform::identity()),
    }
    // frame = the box widened by the fillRect band, then the paint_image fit + draw_pixmap
    self.draw_picture_fill(x, y, w, h, asset_id, stretch, transform, Some(&mask))?;
}
```

```ts
// packages/pptx/src/render/canvas.ts
async function paintShape(ctx, shape, resolveImage): Promise<void> {
  buildPath(ctx, shape.path, shape.x, shape.y, shape.w, shape.h);
  if (shape.fill?.kind === 'picture') {
    const source = shape.fill.assetId && resolveImage ? await resolveImage(shape.fill.assetId) : null;
    if (source) { ctx.save(); ctx.clip(); drawStretched(ctx, source, shape); ctx.restore(); }
  } else if (shape.fill) {
    ctx.fillStyle = paintStyle(ctx, shape.fill, shape.x, shape.y, shape.w, shape.h);
    ctx.fill();
  }
  if (shape.stroke) strokeCurrentPath(ctx, shape.stroke);
}
```

## Risks

- **Ordering.** Landing B before A paints `image1.jpeg` across a full 1097x58 rectangle on all
  three slides, which is very likely a *worse* `fine_pct` than the current blank. Landing A before B
  changes nothing visible on this deck. Ship them together, or land A first and accept that this
  cluster stays open until B.
- **Winding.** The 15-contour path is the corpus's only shape whose correctness depends on contour
  winding. Get it wrong and the letters fill solid — obvious in `evidence-4.png` but easy to miss
  in an aggregate diff number. Add the golden test.
- **Contract version.** A new `Paint` kind is additive JSON but unmatched by an older reader;
  `CONTRACT_VERSION` (`crates/pptx-render/src/display_list.rs:5`) and the assertion at
  `crates/pptx-render/src/lib.rs:405` are the decision point.
- **`ShapeFill` is shared with docx** (`crates/docx-parse/src/text_box.rs:29`,
  `crates/docx-parse/src/drawingml.rs:123`, `crates/docx-parse/src/shape.rs:512`). Widening it
  changes a serialised type two formats persist. `set_fill` / `fill_element`
  (`crates/pptx-parse/src/write.rs:1005`, `:1037`) has no `"picture"` arm and would error if an
  edit ever tried to author one; it is not on the read path, so round-tripping the source XML is
  unaffected, but an explicit error message there is worth adding.
- **`paintShape` becoming async** ripples to any host calling it directly; check
  `packages/pptx` exports before changing the signature.
- **Effects still missing.** The shadow and reflection are unimplemented across all of pptx, so
  this deck's diff will not reach zero and should not be treated as a regression.

## Effort

**Hard.** The custGeom half is a medium port this cluster inherits from
`geometry-custom-collapses-to-bbox`, and on top of it the picture-fill half is a new capability
crossing `pptx-parse`, `ooxml-drawingml`, `pptx-edit`, `pptx-render`'s display-list contract,
`pptx-raster`, and the TypeScript canvas backend — including an async signature change and a
`Paint` variant that every consumer must handle. Every piece has working prior art in `docx-parse`
and `packages/docx`, which is what keeps it from being harder, but the surface is wide and it buys
three findings on one deck: schedule it after the higher-occurrence clusters.
