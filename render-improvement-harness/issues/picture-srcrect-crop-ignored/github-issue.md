# pptx: Picture srcRect crop (and ellipse crop mask) ignored

**Describe the bug**

Every `<p:pic>` paints its whole source bitmap stretched edge to edge into the shape
frame. Two OOXML properties that should reduce what is visible are lost on the way to the
display list:

1. `<a:srcRect>` inside `<p:blipFill>`, which selects the sub-rectangle of the source to
   stretch into the frame. Ignoring it exposes whatever the deck author cropped away —
   an OS X dock along the bottom of a laptop-screen mockup (`evidence-1.png`), the
   transparent margin and dock of a laptop mockup composite (`evidence-2.png`), and a
   tagline row baked under the "ELIXIR" wordmark in the slide-master logo
   (`evidence-3.png`). It also changes scale: the retained artwork shrinks, because the
   discarded strip still consumes frame space.
2. `<a:prstGeom>` inside a picture's `<p:spPr>`, which masks the picture to that preset
   shape. Ignoring it leaves square photos where the deck asks for circles
   (`evidence-4.png`) or rounded rectangles.

Seen on 14 slides across 2 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/08** Picture 8 (`srcRect t="251" b="16720"`): the OS X dock the bottom 16.7% crop should remove is painted inside the laptop screen.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-srcrect-crop-ignored/evidence-1.png)

**2. cisco-cloud-security/17** Picture 2 (`l="6360" r="6360" b="8637"`) and Picture 8 (`t="251" b="16720"`): the laptop's black bezel is replaced by the source's transparent margin, and the dock strip bleeds into the screen.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-srcrect-crop-ignored/evidence-2.png)

**3. project17/04** Slide-master Picture 11 (`l="15358" t="9779" r="21228" b="24405"`): the tagline row under the wordmark is visible and the mark itself renders smaller.

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-srcrect-crop-ignored/evidence-3.png)

**4. project17/10** Pictures 6 and 58 carry `<a:prstGeom prst="ellipse">`; both paint as square, uncropped rectangles.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-srcrect-crop-ignored/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 8, 12, 15, 17, 18, 23
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 4, 6, 7, 10, 12

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(7, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**Confirmed for `srcRect`.** The value is parsed and then dropped one layer later.

- [`crates/pptx-parse/src/drawing.rs:186`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L186) reads `<a:srcRect>` into `Picture::crop`, and
  [`crates/pptx-parse/src/drawing.rs:752`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L752) fills every side of `PictureCrop`
  (field at [`crates/pptx-parse/src/model.rs:216`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L216), type at
  [`crates/pptx-parse/src/model.rs:222`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L222)) in OOXML thousandths of a percent.
- Nothing downstream reads that field. [`crates/pptx-raster/README.md:49`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/README.md#L49) already records
  this: "`PictureCrop` (`srcRect`) is parsed but dropped by the layout pass".
- Both layout paths drop it. Master and layout shapes go through `render_parsed_shape`,
  whose `ShapeNode::Picture` arm at [`crates/pptx-render/src/layout.rs:534`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L534) builds
  `Primitive::Image` from `media_part_path`, `outline` and the rect only — this is the
  project17 master-logo case. Slide shapes go through `render_snapshot_shape`, whose
  `ShapeKind::Picture` arm at [`crates/pptx-render/src/layout.rs:425`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L425) does the same — this
  is the cisco case.
- The snapshot path loses it even earlier: [`crates/pptx-edit/src/deck.rs:139`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L139) seeds a
  picture into the collaborative document with only `kind`, `geometry`, `fillJson`,
  `outlineJson` and `mediaPartPath`, and `ShapeSnapshot`
  ([`crates/pptx-edit/src/model.rs:99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L99)) has no crop field, so
  [`crates/pptx-edit/src/deck.rs:823`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L823) cannot read one back.
- The display-list contract has no place to put it either: `Primitive::Image` at
  [`crates/pptx-render/src/display_list.rs:99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L99) is `{x, y, w, h, asset_id, stroke,
  transform}`. The host-composed path repeats the omission at
  [`crates/pptx-render/src/lib.rs:48`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L48) and [`crates/pptx-render/src/lib.rs:200`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L200).
- Consequently both backends stretch: `paint_image` at
  [`crates/pptx-raster/src/lib.rs:391`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L391) builds `fit` from `frame.width() / source.width()`
  and `frame.height() / source.height()` ([`crates/pptx-raster/src/lib.rs:406`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L406)) and blits
  the whole pixmap; the canvas backend calls the 5-argument
  `ctx.drawImage(source, x, y, w, h)` at [`packages/pptx/src/render/canvas.ts:208`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L208), with
  `ImagePrimitive` ([`packages/pptx/src/types.ts:246`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L246)) carrying no crop.

For contrast, docx already models this end to end: `RelativeRect` at
[`crates/docx-parse/src/shape.rs:68`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/shape.rs#L68), `pictureSrcRect` in the display list at
[`crates/docx-layout/src/display_list.rs:7613`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/display_list.rs#L7613), and `drawCroppedImage` at
[`packages/docx/src/layout/render/canvasBackend.ts:1340`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/docx/src/layout/render/canvasBackend.ts#L1340).

**Confirmed for the geometry mask, and it is a deeper gap: the value is never parsed at
all.** `parse_picture` ([`crates/pptx-parse/src/drawing.rs:158`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L158)) reads `spPr` only for
`xfrm`, `parse_fill` and `parse_outline` ([`crates/pptx-parse/src/drawing.rs:181-188`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L181-L188)); it
never touches `<a:prstGeom>`, and `Picture` ([`crates/pptx-parse/src/model.rs:210`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L210)) has no
`geometry` or `adjust_values` field. The edit snapshot then hardcodes
`shape_map.insert(txn, "geometry", "rect")` for pictures at
[`crates/pptx-edit/src/deck.rs:141`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L141). So a picture's preset shape is lost at the parser, not
at the renderer.

Verified against the decks' XML: `project17/xml/10/slide.xml` has
`<a:prstGeom prst="ellipse">` on Picture 6 and Picture 58, and
`cisco-cloud-security/xml/17/slide.xml` has `<a:prstGeom prst="roundRect">` on Picture
108, so this is not ellipse-only.

Two smaller points, both flagged as hypotheses:

- `integer_attribute` ([`crates/pptx-parse/src/drawing.rs:942`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L942)) parses into `i32`, so a
  negative `srcRect` (an outset, legal in `ST_Percentage`) survives parsing. Whatever
  consumes the crop has to clamp or letterbox rather than index outside the source.
- Some project17 pictures also sit a few pixels off horizontally from the reference
  (visible in `evidence-4.png`). That offset is not explained by the crop and is probably
  a separate transform issue; not investigated here.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Carry the crop through the four layers that currently drop it, then mask.

1. **Contract.** Add an optional `crop` to `Primitive::Image`
   ([`crates/pptx-render/src/display_list.rs:99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L99)) as four `f32` fractions in `0..1`, and the
   matching optional `geometry` + `path` so a picture can be clipped to a preset shape.
   Skip-serialize both so a rect picture with no crop keeps today's JSON and
   `CONTRACT_VERSION` ([`crates/pptx-render/src/display_list.rs:5`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L5)) does not have to move;
   confirm against the contract fixtures before deciding to leave it at 1. Mirror the
   fields on `ImagePrimitive` ([`packages/pptx/src/types.ts:246`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L246)).

2. **Parse.** `parse_picture` ([`crates/pptx-parse/src/drawing.rs:158`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L158)) already stores the
   crop; add `geometry` and `adjust_values` read from `spPr/prstGeom`, reusing whatever
   `parse_shape` does for the same element so preset names and `<a:avLst>` stay consistent.

3. **Snapshot.** Replace the hardcoded `"rect"` at [`crates/pptx-edit/src/deck.rs:141`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L141) with
   the parsed geometry, seed `adjustValuesJson` the way the `Shape` arm does, and add
   `cropJson`. Add `crop: PictureCrop` to `ShapeSnapshot`
   ([`crates/pptx-edit/src/model.rs:99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L99)) and read it back at
   [`crates/pptx-edit/src/deck.rs:823`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L823). Round-trip is safe: `save.rs` only serializes newly
   added shapes and rejects non-`Shape` kinds ([`crates/pptx-edit/src/save.rs:412`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L412)), so no
   writer needs to learn `srcRect`.

4. **Layout.** Both picture arms ([`crates/pptx-render/src/layout.rs:425`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L425) and
   [`crates/pptx-render/src/layout.rs:534`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L534)) convert the thousandths-of-a-percent crop to
   fractions and, when the geometry is not `rect`, attach the same
   `geometry_path(...)` ([`crates/pptx-render/src/layout.rs:1946`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1946)) the shape arm already
   builds. Do the same in the host-composed path
   ([`crates/pptx-render/src/lib.rs:48`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L48), [`crates/pptx-render/src/lib.rs:200`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L200)).

5. **Raster.** In `paint_image` ([`crates/pptx-raster/src/lib.rs:391`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L391)), scale the fit
   transform by the kept fraction and translate by the discarded left/top, then draw
   through a mask: intersect the existing `clip` with the frame rect (or with the
   geometry path when present) using the same machinery as `clipped`
   ([`crates/pptx-raster/src/lib.rs:325`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L325)), which is what stops the outsized source spilling
   past the frame.

6. **Canvas.** Switch [`packages/pptx/src/render/canvas.ts:208`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L208) to the 9-argument
   `drawImage`, guarded by a `ctx.save()`/`clip()`/`restore()` when the primitive carries a
   path. [`packages/docx/src/layout/render/canvasBackend.ts:1340`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/docx/src/layout/render/canvasBackend.ts#L1340) (`drawCroppedImage`) is
   the reference implementation, including its negative-crop clamping.

```rust
// crates/pptx-raster/src/lib.rs, in paint_image
let (kx, ky) = (1.0 - crop.left - crop.right, 1.0 - crop.top - crop.bottom);
if kx <= 0.0 || ky <= 0.0 { return Ok(()); }        // fully cropped away
let (sw, sh) = (source.width() as f32, source.height() as f32);
let fit = Transform::from_row(
    frame.width() / (sw * kx), 0.0,
    0.0, frame.height() / (sh * ky),
    frame.x() - crop.left * sw * frame.width() / (sw * kx),
    frame.y() - crop.top  * sh * frame.height() / (sh * ky),
);
// the source now overhangs the frame, so the blit must be masked
let mask = self.clipped_to(clip, frame, path.as_deref(), transform)?;
self.pixmap.draw_pixmap(0, 0, source.as_ref(), &paint,
                        transform.pre_concat(fit), mask.as_ref());
```

```rust
// crates/pptx-parse/src/drawing.rs, parse_picture
geometry: properties
    .and_then(|value| value.child("prstGeom"))
    .and_then(|value| value.attribute("prst"))
    .unwrap_or("rect")
    .to_owned(),
```

Risks and tests to add:

- **Masking is now mandatory for pictures.** Today an image is blitted unmasked because it
  exactly fills its frame; after the fix the source overhangs, so a missing mask turns a
  crop into a bleed over neighbouring shapes. Every path that paints an image needs the
  mask, including the rotated case — `Mask` is built in device space, so the frame path
  must be transformed first, exactly as [`crates/pptx-raster/src/lib.rs:337`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L337) does.
- **Negative `srcRect`** (outset) parses fine into `i32`
  ([`crates/pptx-parse/src/drawing.rs:942`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L942)) but would make the kept fraction exceed 1 and
  place the source inside the frame with a gap. Clamp, and add a parse test.
- **Divide by zero / degenerate crops** when the four sides sum to 100%; guard before
  computing the fit, as the sketch does.
- **Pictures gaining a real geometry** changes `ShapeSnapshot::geometry` for every picture
  from the constant `"rect"`. Anything asserting that constant, and any host reading the
  snapshot, sees a new value — grep the edit and wasm tests before changing it.
- **Contract churn.** If the fixtures pin exact JSON, the new optional fields need the
  fixtures regenerated even though they are skip-serialized when absent.
- Tests to add: a `pptx-parse` unit test for `PictureCrop` and for a picture's
  `prstGeom`; a cropped and an ellipse-masked golden beside `golden_image`
  ([`crates/pptx-raster/tests/golden.rs:283`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/tests/golden.rs#L283)); a layout assertion that
  `Primitive::Image` carries the crop on both the snapshot and the parsed-shape path
  (master pictures only reach the parsed path).

**How to verify**

Re-render `cisco-cloud-security` slides 08, 12, 15, 17, 18, 23 and `project17` slides 02,
04, 06, 07, 10, 12 with
`.venv/bin/python render-improvement-harness/scripts/render_bo.py <deck>` followed by
`diff.py <deck>`. The cisco laptop-mockup slides (08, 17, 18) carry most of the pixel cost
of this cluster; their `fine_pct` should drop noticeably once the dock and bezel strips
disappear. `project17` slides move less — the master logo is a small corner shape — but
slide 10's two portraits should become circles.

Existing coverage to extend: `golden_image` at [`crates/pptx-raster/tests/golden.rs:283`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/tests/golden.rs#L283) is
the only picture golden and it paints an uncropped checker; add a cropped and a masked
variant beside it. `crates/pptx-parse` has no `srcRect` test at all — the only occurrence
of that string under `crates/` outside docx is the parser line itself — so a parse test
for `PictureCrop` and for a picture's `prstGeom` is new ground.
[`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2008) holds the layout unit tests, where an assertion
that the emitted `Primitive::Image` carries the crop belongs.

**Additional context**

none.

Related issues found in the same run: none.

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-edit/src/deck.rs`, `crates/pptx-edit/src/model.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/lib.rs`, `crates/pptx-render/src/display_list.rs`, `crates/pptx-raster/src/lib.rs`, `crates/pptx-raster/README.md`, `packages/pptx/src/types.ts`, `packages/pptx/src/render/canvas.ts`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/picture-srcrect-crop-ignored/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
