---
id: unsupported-custgeom-picturefill-wordmark-not-drawn
title: Resolve a:blipFill on a p:sp so a picture-filled freeform paints instead of vanishing
category: unsupported-element
impact: low
effort: hard
confidence: high
status: open
occurrences: 3
decks: [typography-trick]
findings: [typography-trick/01/1, typography-trick/02/1, typography-trick/03/1]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/ooxml-drawingml/src/shape.rs, crates/pptx-edit/src/deck.rs, crates/pptx-render/src/display_list.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/lib.rs, crates/pptx-raster/src/lib.rs, packages/pptx/src/types.ts, packages/pptx/src/render/canvas.ts]
---

## Symptom

Every slide in `typography-trick` is a single decorative element: the wordmark "CREATIVE VENUS",
drawn as one `p:sp` whose `a:custGeom` traces 15 letterform contours and whose fill is an
`a:blipFill` pointing at `ppt/media/image1.jpeg`, so each glyph shows a different slice of the
photo. The candidate draws **nothing at all** in that band — not the letters, not a bounding-box
rectangle, not a placeholder (`evidence-1.png`, `evidence-2.png`, `evidence-3.png`). All three
slides carry the byte-identical shape and fail identically. `evidence-4.png` zooms "CREA" to show
what is lost: the photographic texture, and the counters punched out of C, R and A.

This is **not** the same failure as `geometry-custom-collapses-to-bbox`. There, a `custGeom`
collapses to a rectangle but is still painted in the shape's own fill. Here the shape reaches the
display list and then paints zero pixels, because a `blipFill` on a `p:sp` resolves to no paint at
all. Fixing custGeom parsing alone leaves this deck exactly as blank as it is today; fixing the
picture fill alone paints the photo across the full 1097x58 bounding rectangle. **Both halves are
required for these three findings.**

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | typography-trick/01 | Reference / candidate / diff band across `Freeform: Shape 5` (id 6). The wordmark and its shadow are present in LibreOffice, absent in BetterOffice, and the whole glyph run lights up red in the diff. |
| 2 | typography-trick/02 | Same shape on the dark variant of the slide — same complete miss, so the failure does not depend on background or theme. |
| 3 | typography-trick/03 | Same shape again; the red disc in the diff panel is the unrelated flat-background delta from `fill-nonsolid-fill-types-not-resolved`. |
| 4 | typography-trick/01 | 3x zoom on "CREA", reference above and candidate below: the picture texture inside each glyph and the reverse-wound counters that must stay transparent. |

## Root cause (hypothesis)

**Confirmed, and reproduced against the real display list.** Rendering slide 1 through the same
entry point the harness uses (`Presentation::render_slide`,
`crates/betteroffice-pptx/src/presentation.rs:312`) emits this primitive for shape id 6:

```json
{ "kind": "shape", "objectId": 6, "name": "Freeform: Shape 5",
  "x": 87.07, "y": 295.29, "w": 1097.15, "h": 57.99,
  "geometry": "custom",
  "path": [ {"type":"move","x":0,"y":0}, {"type":"line","x":1,"y":0},
            {"type":"line","x":1,"y":1}, {"type":"line","x":0,"y":1}, {"type":"close"} ] }
```

No `fill` key and no `stroke` key. The shape is emitted, sized and positioned correctly, and then
draws nothing. Two independent gaps produce that:

### 1. `a:blipFill` on a `p:sp` is recognised but its image is thrown away

`parse_fill` (`crates/pptx-parse/src/drawing.rs:565`) ends its chain with

```rust
if element.child("blipFill").is_some() {
    return Some(ShapeFill::named("picture"));
}
```

(`crates/pptx-parse/src/drawing.rs:579-581`). `ShapeFill::named` builds `{ fill_type, color: None,
gradient: None }` (`crates/ooxml-drawingml/src/shape.rs:17-23`), and `ShapeFill`
(`crates/ooxml-drawingml/src/shape.rs:7-14`) has no field that could hold a blip at all — no
relationship id, no `srcRect`, no `stretch`/`fillRect`, no `tile`. The `r:embed="rId2"` and the
`<a:stretch><a:fillRect l="-53000"/>` in the XML are read and discarded.

`parse_shape` (`crates/pptx-parse/src/drawing.rs:138`) could not resolve the id even if the field
existed: unlike `parse_picture` (`crates/pptx-parse/src/drawing.rs:159`), which takes
`relationships` and calls `relationship_target` (`crates/pptx-parse/src/drawing.rs:930`) to produce
`Picture::media_part_path` (`crates/pptx-parse/src/model.rs:215`), `parse_shape` never receives the
relationship table. Its single call site already has one in scope
(`crates/pptx-parse/src/drawing.rs:110`), so this is a signature change, not a plumbing problem.

`layout::paint` (`crates/pptx-render/src/layout.rs:1897`) then does the visible damage: `"picture"`
is not `"none"`, there is no gradient, and `fill.color` is `None`, so it falls through to
`resolve_color_value_to_hex_with_theme(None, ..)` and returns `None`. That `None` is what both emit
sites store — `crates/pptx-render/src/layout.rs:384-388` and `:420` for the snapshot path,
`crates/pptx-render/src/layout.rs:526` for the parsed path (a third caller,
`crates/pptx-render/src/layout.rs:183`, resolves the slide background the same way). The shape's
outline is
`<a:ln><a:noFill/>`, which `parse_outline` turns into `None`
(`crates/pptx-parse/src/drawing.rs:626-628`), so no stroke rescues it either. In the raster,
`paint_shape` (`crates/pptx-raster/src/lib.rs:364-387`) builds the path and then skips both the fill
and the stroke block. Zero pixels, no error, no `skipped_images` increment — matching
`bo-log.json`'s clean log for all three slides.

The contract below layout has no way to express this even once parsing is fixed:

- `Paint` (`crates/pptx-render/src/display_list.rs:24-34`) is `Solid | Gradient` only.
- `Primitive::Shape` (`crates/pptx-render/src/display_list.rs:79-99`) carries no `asset_id`;
  only `Primitive::Image` (`crates/pptx-render/src/display_list.rs:99-114`) does, and that
  primitive always paints an axis-aligned rectangle
  (`crates/pptx-raster/src/lib.rs:391-431`, `packages/pptx/src/render/canvas.ts:201-214`).
- `packages/pptx/src/types.ts:205-212` mirrors the two-variant union.

The snapshot path is the one exception that is already half-built: `ShapeSnapshot.media_part_path`
exists (`crates/pptx-edit/src/model.rs:118`) and is read back for **every** shape kind
(`crates/pptx-edit/src/deck.rs:823`), but `seed_shape` only ever writes `mediaPartPath` in the
`ShapeNode::Picture` arm (`crates/pptx-edit/src/deck.rs:139-147`); the `ShapeNode::Shape` arm
(`crates/pptx-edit/src/deck.rs:123-137`) writes `geometry`, `adjustValuesJson`, `fillJson` and
`outlineJson` and nothing else. So carrying the media path for a picture-filled `p:sp` needs a
write in one arm and **no `SCHEMA_VERSION` bump** (`crates/pptx-edit/src/deck.rs:23`) — the key is
already part of the document shape. That is materially cheaper than the `geometryPathJson` field
the geometry cluster needs, and the two changes should ship in one schema revision.

### 2. The `custGeom` outline is never parsed

Everything in `geometry-custom-collapses-to-bbox` applies verbatim: `parse_geometry`
(`crates/pptx-parse/src/drawing.rs:335`) returns the string `"custom"` and never reads `a:pathLst`,
and `geometry_path` (`crates/pptx-render/src/layout.rs:1946-1957`) falls back to the `"rect"`
preset. The four-command rectangle in the dump above is that fallback. See that issue for the port
plan; this report does not restate it.

What this shape adds to that cluster's measurements:

- One `<a:path w="10450353" h="552355">`, **15 subpaths** (15 `moveTo` / 15 `close`) inside it:
  13 glyphs plus the counters of R and A. The geometry cluster measured "one `a:path` per
  `pathLst`" across the corpus and that holds here, but "one contour per path" does not — this is
  the corpus's clearest multi-contour case.
- 297 commands (96 `lnTo`, 186 `cubicBezTo`, 15 `moveTo`, 15 `close`), 669 `a:pt`, all numeric,
  zero `arcTo`, zero `quadBezTo`. Within the `moveTo`/`lnTo`/`cubicBezTo`/`close` subset that
  cluster already scopes.
- The counters must be reverse-wound relative to their glyph outlines for `FillRule::Winding`
  (`crates/pptx-raster/src/lib.rs:382`) to punch them out. `evidence-4.png` is the reference for
  that check; if the winding is wrong the letters fill solid and the deck looks worse, not better.

### What will still be missing after the fix

The shape carries `<a:effectLst>` with an `outerShdw blurRad="127000"` and a `reflection`. Grepping
`crates/pptx-parse/src`, `crates/pptx-render/src` and `crates/pptx-raster/src` for `effectLst`,
`outerShdw` or `reflection` returns only `crates/pptx-parse/src/write.rs:1003`, where `effectLst`
appears in a list of elements to write *after* a fill. **pptx has no effect support anywhere**, so
the shadow and the reflection stay absent and the diff will not reach zero. Not tracked here.

### Not confirmed, but found while dumping the display list

The sibling background finding (`typography-trick/01/2` etc., cluster
`fill-nonsolid-fill-types-not-resolved`) is **not** a parse failure. The slide-1 display list
carries a correct radial paint:

```json
"background": { "kind": "gradient", "gradientType": "radial",
  "stops": [ {"position": 1.0, "color": "#D6DCE5"}, {"position": 0.0, "color": "#FFFFFF"} ] }
```

Note the stops are in **document order, descending** — `pos="100000"` before `pos="0"` — because
`parse_gradient_fill` (`crates/pptx-parse/src/drawing.rs:585-621`) preserves XML order and `paint`
(`crates/pptx-render/src/layout.rs:1902-1921`) does not sort. `gradient_paint`
(`crates/pptx-raster/src/lib.rs:627-692`) hands that straight to `tiny_skia::RadialGradient`.
Unsorted stops collapsing to the first color is a plausible explanation for the flat outer-stop
fill the reports sampled, but I did not verify tiny_skia's behaviour. **Hypothesis, for that
cluster's owner.**

## Verification

Re-render with `.venv/bin/python render-improvement-harness/scripts/render_bo.py typography-trick`
then `diff.py typography-trick`. All three slides are single-shape slides, so the read is
unambiguous:

- `fine_pct` should fall from 2.88 / 3.54 / 6.52 toward the residual left by the missing shadow and
  reflection plus the background gradient, which is a separate cluster.
- The `r2c*` hot cells — 8.4-12.6% on 01, 12.1-14.5% on 02 — are entirely this shape's bounding
  band and should collapse.
- Slide 02's `coarse_pct` of 5.12 (the only `major` coarse verdict in the deck) is this shape.

Check `evidence-4.png` against the new output at 3x: the counters of C, R and A must be background,
not photo, and the texture inside each glyph must be a different slice of `image1.jpeg`. The
`<a:stretch><a:fillRect l="-53000"/>` means the image is drawn into a band 1.53x the shape width
starting 53% of the width to the left of it; ignoring it still paints the letters but shifts and
squashes the colours, so compare hues per glyph rather than just "something is drawn".

Coverage to extend:

- `crates/pptx-parse/src/drawing.rs` has no `blipFill`-on-`p:sp` test; the only `blipFill` reads are
  `crates/pptx-parse/src/drawing.rs:167` (`p:pic`) and `:579` (this branch). A parse test asserting
  the resolved media part path for a `p:sp` is new ground.
- `crates/pptx-raster/tests/golden.rs:283` (`golden_image`) already exercises the decoder and the
  `AssetMap` fixture at `crates/pptx-raster/tests/golden.rs:76-78`. A `golden_picture_filled_shape`
  beside it, using a non-convex two-contour path, pins both the picture fill and the winding rule.
- `crates/pptx-render/src/layout.rs:2403-2410` asserts on `geometry == "custom"` primitives and
  their `Paint::Solid` fill; a picture-fill variant must not disturb that chart-side assertion, nor
  `crates/pptx-render/src/chart.rs:485` / `:594`.
- `crates/pptx-render/src/lib.rs:405` asserts the composed contract version. Adding a `Paint`
  variant is additive on the wire but a `kind` an older consumer cannot match — decide explicitly
  whether `CONTRACT_VERSION` (`crates/pptx-render/src/display_list.rs:5`) moves.
