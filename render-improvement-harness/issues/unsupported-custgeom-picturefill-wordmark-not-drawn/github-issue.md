# pptx: custGeom shape with a picture fill not drawn at all

**Describe the bug**

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

Seen on 3 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact low, estimated effort hard, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. typography-trick/01** Reference / candidate / diff band across `Freeform: Shape 5` (id 6). The wordmark and its shadow are present in LibreOffice, absent in BetterOffice, and the whole glyph run lights up red in the diff.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-custgeom-picturefill-wordmark-not-drawn/evidence-1.png)

**2. typography-trick/02** Same shape on the dark variant of the slide — same complete miss, so the failure does not depend on background or theme.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-custgeom-picturefill-wordmark-not-drawn/evidence-2.png)

**3. typography-trick/03** Same shape again; the red disc in the diff panel is the unrelated flat-background delta from `fill-nonsolid-fill-types-not-resolved`.

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-custgeom-picturefill-wordmark-not-drawn/evidence-3.png)

**4. typography-trick/01** 3x zoom on "CREA", reference above and candidate below: the picture texture inside each glyph and the reverse-wound counters that must stay transparent.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-custgeom-picturefill-wordmark-not-drawn/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `Trick To Create Beautiful Typography in Microsoft Office PowerPoint PPT.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 2, 3

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(0, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**Confirmed, and reproduced against the real display list.** Rendering slide 1 through the same
entry point the harness uses (`Presentation::render_slide`,
[`crates/betteroffice-pptx/src/presentation.rs:312`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/betteroffice-pptx/src/presentation.rs#L312)) emits this primitive for shape id 6:

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

`parse_fill` ([`crates/pptx-parse/src/drawing.rs:565`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L565)) ends its chain with

```rust
if element.child("blipFill").is_some() {
    return Some(ShapeFill::named("picture"));
}
```

([`crates/pptx-parse/src/drawing.rs:579-581`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L579-L581)). `ShapeFill::named` builds `{ fill_type, color: None,
gradient: None }` ([`crates/ooxml-drawingml/src/shape.rs:17-23`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/shape.rs#L17-L23)), and `ShapeFill`
([`crates/ooxml-drawingml/src/shape.rs:7-14`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/shape.rs#L7-L14)) has no field that could hold a blip at all — no
relationship id, no `srcRect`, no `stretch`/`fillRect`, no `tile`. The `r:embed="rId2"` and the
`<a:stretch><a:fillRect l="-53000"/>` in the XML are read and discarded.

`parse_shape` ([`crates/pptx-parse/src/drawing.rs:138`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L138)) could not resolve the id even if the field
existed: unlike `parse_picture` ([`crates/pptx-parse/src/drawing.rs:159`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L159)), which takes
`relationships` and calls `relationship_target` ([`crates/pptx-parse/src/drawing.rs:930`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L930)) to produce
`Picture::media_part_path` ([`crates/pptx-parse/src/model.rs:215`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L215)), `parse_shape` never receives the
relationship table. Its single call site already has one in scope
([`crates/pptx-parse/src/drawing.rs:110`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L110)), so this is a signature change, not a plumbing problem.

`layout::paint` ([`crates/pptx-render/src/layout.rs:1897`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1897)) then does the visible damage: `"picture"`
is not `"none"`, there is no gradient, and `fill.color` is `None`, so it falls through to
`resolve_color_value_to_hex_with_theme(None, ..)` and returns `None`. That `None` is what both emit
sites store — [`crates/pptx-render/src/layout.rs:384-388`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L384-L388) and `:420` for the snapshot path,
[`crates/pptx-render/src/layout.rs:526`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L526) for the parsed path (a third caller,
[`crates/pptx-render/src/layout.rs:183`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L183), resolves the slide background the same way). The shape's
outline is
`<a:ln><a:noFill/>`, which `parse_outline` turns into `None`
([`crates/pptx-parse/src/drawing.rs:626-628`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L626-L628)), so no stroke rescues it either. In the raster,
`paint_shape` ([`crates/pptx-raster/src/lib.rs:364-387`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L364-L387)) builds the path and then skips both the fill
and the stroke block. Zero pixels, no error, no `skipped_images` increment — matching
`bo-log.json`'s clean log for all three slides.

The contract below layout has no way to express this even once parsing is fixed:

- `Paint` ([`crates/pptx-render/src/display_list.rs:24-34`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L24-L34)) is `Solid | Gradient` only.
- `Primitive::Shape` ([`crates/pptx-render/src/display_list.rs:79-99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L79-L99)) carries no `asset_id`;
  only `Primitive::Image` ([`crates/pptx-render/src/display_list.rs:99-114`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L99-L114)) does, and that
  primitive always paints an axis-aligned rectangle
  ([`crates/pptx-raster/src/lib.rs:391-431`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L391-L431), [`packages/pptx/src/render/canvas.ts:201-214`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L201-L214)).
- [`packages/pptx/src/types.ts:205-212`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L205-L212) mirrors the two-variant union.

The snapshot path is the one exception that is already half-built: `ShapeSnapshot.media_part_path`
exists ([`crates/pptx-edit/src/model.rs:118`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L118)) and is read back for **every** shape kind
([`crates/pptx-edit/src/deck.rs:823`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L823)), but `seed_shape` only ever writes `mediaPartPath` in the
`ShapeNode::Picture` arm ([`crates/pptx-edit/src/deck.rs:139-147`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L139-L147)); the `ShapeNode::Shape` arm
([`crates/pptx-edit/src/deck.rs:123-137`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L123-L137)) writes `geometry`, `adjustValuesJson`, `fillJson` and
`outlineJson` and nothing else. So carrying the media path for a picture-filled `p:sp` needs a
write in one arm and **no `SCHEMA_VERSION` bump** ([`crates/pptx-edit/src/deck.rs:23`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L23)) — the key is
already part of the document shape. That is materially cheaper than the `geometryPathJson` field
the geometry cluster needs, and the two changes should ship in one schema revision.

### 2. The `custGeom` outline is never parsed

Everything in `geometry-custom-collapses-to-bbox` applies verbatim: `parse_geometry`
([`crates/pptx-parse/src/drawing.rs:335`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L335)) returns the string `"custom"` and never reads `a:pathLst`,
and `geometry_path` ([`crates/pptx-render/src/layout.rs:1946-1957`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1946-L1957)) falls back to the `"rect"`
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
  ([`crates/pptx-raster/src/lib.rs:382`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L382)) to punch them out. `evidence-4.png` is the reference for
  that check; if the winding is wrong the letters fill solid and the deck looks worse, not better.

### What will still be missing after the fix

The shape carries `<a:effectLst>` with an `outerShdw blurRad="127000"` and a `reflection`. Grepping
`crates/pptx-parse/src`, `crates/pptx-render/src` and `crates/pptx-raster/src` for `effectLst`,
`outerShdw` or `reflection` returns only [`crates/pptx-parse/src/write.rs:1003`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1003), where `effectLst`
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
`parse_gradient_fill` ([`crates/pptx-parse/src/drawing.rs:585-621`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L585-L621)) preserves XML order and `paint`
([`crates/pptx-render/src/layout.rs:1902-1921`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1902-L1921)) does not sort. `gradient_paint`
([`crates/pptx-raster/src/lib.rs:627-692`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L627-L692)) hands that straight to `tiny_skia::RadialGradient`.
Unsorted stops collapsing to the first color is a plausible explanation for the flat outer-stop
fill the reports sampled, but I did not verify tiny_skia's behaviour. **Hypothesis, for that
cluster's owner.**

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Two changes must land together; either alone leaves `typography-trick` wrong.

**A. The custGeom outline** — exactly the port described in
`render-improvement-harness/issues/geometry-custom-collapses-to-bbox/possible-solution.md`.
Nothing to add here except one constraint that cluster's corpus scan did not surface: this shape is
a *single* `a:path` holding 15 subpaths, so the walker must keep emitting after a `close` rather
than stopping at the first contour, and must not reorder or reverse contours — the counters of C, R
and A are punched out by `FillRule::Winding` ([`crates/pptx-raster/src/lib.rs:382`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L382)) and by canvas's
default nonzero rule ([`packages/pptx/src/render/canvas.ts:126-152`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L126-L152)).

**B. A picture fill on a `p:sp`** — new capability, six crates plus the web package. Follow the
docx implementation, which already does all of this end to end:

1. **`crates/pptx-parse`** — give `parse_shape` ([`crates/pptx-parse/src/drawing.rs:138`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L138)) the
   `relationships` slice its call site already holds
   ([`crates/pptx-parse/src/drawing.rs:110`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L110)), and replace the bare
   `ShapeFill::named("picture")` at [`crates/pptx-parse/src/drawing.rs:579-581`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L579-L581) with a parse that
   captures `a:blip/@r:embed`, resolves it through `relationship_target`
   ([`crates/pptx-parse/src/drawing.rs:930`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L930)), and records `a:srcRect` plus the
   `a:stretch/a:fillRect` (or `a:tile`) mode. [`crates/docx-parse/src/shape.rs:620-636`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/shape.rs#L620-L636) is the exact
   shape of this code, including the `r:embed` / bare-`embed` fallback and
   `parse_picture_fill_mode` ([`crates/docx-parse/src/shape.rs:651`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/shape.rs#L651)).
   `parse_background` ([`crates/pptx-parse/src/drawing.rs:554`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L554)) shares `parse_fill`, so a
   picture slide background comes along for free once the relationships reach it.

2. **Where the payload lives.** `ShapeFill` ([`crates/ooxml-drawingml/src/shape.rs:7-14`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/shape.rs#L7-L14)) is shared
   with docx ([`crates/docx-parse/src/text_box.rs:29`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/text_box.rs#L29), [`crates/docx-parse/src/drawingml.rs:123`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L123)), so
   widening it touches both formats' serialised models. Two options; pick one and say so in the PR:
   - Add `picture: Option<PictureFill>` to `ShapeFill` with
     `#[serde(default, skip_serializing_if = "Option::is_none")]` — one type, docx can adopt it
     later.
   - Or keep `ShapeFill` untouched and hang `picture_fill: Option<PictureFill>` off
     `pptx_parse::Shape` ([`crates/pptx-parse/src/model.rs:198-207`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L198-L207)) beside the geometry path the
     other cluster adds. Cheaper blast radius, one more field to thread.
   `PictureFill` needs `media_part_path`, `relationship_id`, `src_rect` and the stretch rect —
   [`crates/docx-parse/src/shape.rs:36-70`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/shape.rs#L36-L70) (`ShapeFillPaint`) is the reference field set.

3. **`crates/pptx-edit`** — in the `ShapeNode::Shape` arm of `seed_shape`
   ([`crates/pptx-edit/src/deck.rs:123-137`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L123-L137)) write `mediaPartPath` when the fill is a picture, the
   way the `ShapeNode::Picture` arm already does at [`crates/pptx-edit/src/deck.rs:144-146`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L144-L146).
   `ShapeSnapshot.media_part_path` ([`crates/pptx-edit/src/model.rs:118`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L118)) and the read-back
   ([`crates/pptx-edit/src/deck.rs:823`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L823)) are kind-agnostic already, so **this needs no
   `SCHEMA_VERSION` bump on its own**; if it ships with the geometry cluster's `geometryPathJson`,
   both ride that one bump.

4. **`crates/pptx-render`** — add a picture arm to `Paint`
   ([`crates/pptx-render/src/display_list.rs:24-34`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L24-L34)) carrying the asset id and the placement rects,
   and emit it from `paint` ([`crates/pptx-render/src/layout.rs:1897`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1897)). `paint` currently takes only
   `(&ShapeFill, &Theme)`; it needs the snapshot's / node's media path too, so give it a third
   argument at all three call sites ([`crates/pptx-render/src/layout.rs:388`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L388), `:526` and, for a
   picture slide background, `:183`). The
   composed path ([`crates/pptx-render/src/lib.rs:161-190`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L161-L190)) deserialises `fill: Option<Paint>`
   straight from JSON and needs no code change.

5. **`crates/pptx-raster`** — in `paint_shape` ([`crates/pptx-raster/src/lib.rs:364`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L364)), branch before
   `shader_paint`: for a picture paint, build a `Mask` from the already-built `Path` the way
   `clipped` does ([`crates/pptx-raster/src/lib.rs:346-359`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L346-L359)), intersect it with the incoming `clip`,
   then reuse the decode-and-fit block from `paint_image`
   ([`crates/pptx-raster/src/lib.rs:401-425`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L401-L425)) with the frame widened by the stretch rect. A
   `tiny_skia::Pattern` shader inside `shader_paint` is the tidier alternative but fights the
   `Paint<'static>` return type, since the pattern borrows the decoded pixmap. Count an
   undecodable blip in `skipped_images` ([`crates/pptx-raster/src/lib.rs:426`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L426)) rather than dropping
   the shape silently — that is the bug being fixed.

6. **`packages/pptx`** — extend the `Paint` union ([`packages/pptx/src/types.ts:205-212`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L205-L212)) and teach
   `paintShape` ([`packages/pptx/src/render/canvas.ts:117`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L117)) to clip to the built path and
   `drawImage` through the resolver. `paintShape` is currently sync while `resolveImage` is async
   ([`packages/pptx/src/render/canvas.ts:15-17`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L15-L17)), so `paintShape` becomes `async` and its call site
   ([`packages/pptx/src/render/canvas.ts:71-72`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L71-L72)) gains an `await`. `drawPictureShapeFill`
   ([`packages/docx/src/layout/render/canvasBackend.ts:704-735`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/docx/src/layout/render/canvasBackend.ts#L704-L735)) is the working version of exactly
   this, including the fillRect band arithmetic and the fall-back-to-solid path when the resolver
   returns nothing.

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

Risks and tests to add:

- **Ordering.** Landing B before A paints `image1.jpeg` across a full 1097x58 rectangle on all
  three slides, which is very likely a *worse* `fine_pct` than the current blank. Landing A before B
  changes nothing visible on this deck. Ship them together, or land A first and accept that this
  cluster stays open until B.
- **Winding.** The 15-contour path is the corpus's only shape whose correctness depends on contour
  winding. Get it wrong and the letters fill solid — obvious in `evidence-4.png` but easy to miss
  in an aggregate diff number. Add the golden test.
- **Contract version.** A new `Paint` kind is additive JSON but unmatched by an older reader;
  `CONTRACT_VERSION` ([`crates/pptx-render/src/display_list.rs:5`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L5)) and the assertion at
  [`crates/pptx-render/src/lib.rs:405`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L405) are the decision point.
- **`ShapeFill` is shared with docx** ([`crates/docx-parse/src/text_box.rs:29`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/text_box.rs#L29),
  [`crates/docx-parse/src/drawingml.rs:123`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L123), [`crates/docx-parse/src/shape.rs:512`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/shape.rs#L512)). Widening it
  changes a serialised type two formats persist. `set_fill` / `fill_element`
  ([`crates/pptx-parse/src/write.rs:1005`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1005), `:1037`) has no `"picture"` arm and would error if an
  edit ever tried to author one; it is not on the read path, so round-tripping the source XML is
  unaffected, but an explicit error message there is worth adding.
- **`paintShape` becoming async** ripples to any host calling it directly; check
  `packages/pptx` exports before changing the signature.
- **Effects still missing.** The shadow and reflection are unimplemented across all of pptx, so
  this deck's diff will not reach zero and should not be treated as a regression.

**How to verify**

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
  [`crates/pptx-parse/src/drawing.rs:167`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L167) (`p:pic`) and `:579` (this branch). A parse test asserting
  the resolved media part path for a `p:sp` is new ground.
- [`crates/pptx-raster/tests/golden.rs:283`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/tests/golden.rs#L283) (`golden_image`) already exercises the decoder and the
  `AssetMap` fixture at [`crates/pptx-raster/tests/golden.rs:76-78`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/tests/golden.rs#L76-L78). A `golden_picture_filled_shape`
  beside it, using a non-convex two-contour path, pins both the picture fill and the winding rule.
- [`crates/pptx-render/src/layout.rs:2403-2410`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2403-L2410) asserts on `geometry == "custom"` primitives and
  their `Paint::Solid` fill; a picture-fill variant must not disturb that chart-side assertion, nor
  [`crates/pptx-render/src/chart.rs:485`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L485) / `:594`.
- [`crates/pptx-render/src/lib.rs:405`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L405) asserts the composed contract version. Adding a `Paint`
  variant is additive on the wire but a `kind` an older consumer cannot match — decide explicitly
  whether `CONTRACT_VERSION` ([`crates/pptx-render/src/display_list.rs:5`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L5)) moves.

**Additional context**

none.

Related issues found in the same run: `fill-nonsolid-fill-types-not-resolved`, `geometry-custom-collapses-to-bbox`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/ooxml-drawingml/src/shape.rs`, `crates/pptx-edit/src/deck.rs`, `crates/pptx-render/src/display_list.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/lib.rs`, `crates/pptx-raster/src/lib.rs`, `packages/pptx/src/types.ts`, `packages/pptx/src/render/canvas.ts`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-custgeom-picturefill-wordmark-not-drawn/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
