# pptx: Blip color effects (duotone, biLevel) not applied or crash the picture

**Describe the bug**

`<a:blip>` can carry colour-transform children that recolour the bitmap before it is
drawn. BetterOffice paints the raw source bytes instead, so every one of these pictures
keeps its original brand colours where the deck asked for a flat monochrome mark: the
Yammer glyph stays teal and the Google Drive glyph stays green/yellow/blue inside their
dark app circles (`evidence-1.png`, `evidence-2.png`).

The cluster's second half — "the picture disappears entirely" — **is not a drop**. In all
three cases the picture is decoded and blitted normally; it just lands in its native
colour on a background of nearly the same colour, so it reads as absent. The Dropbox mark
is blue `#007DE4` on a blue cloud (`evidence-3.png`) and the elastica leaf is cyan
`#03A7DF` on a cyan band (`evidence-4.png`); `biLevel` would have pushed the first to
white and the second to black.

Seen on 3 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact low, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/02** Picture 10 (`biLevel thresh="25000"`, Yammer) renders teal instead of white, and Picture 6 (`duotone` bg2→white, Google Drive) renders full-colour instead of grey-to-white.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-blip-duotone-bilevel-not-applied/evidence-1.png)

**2. cisco-cloud-security/11** The app-logo row: Google Drive keeps its brand colours, and the OneDrive mark (`biLevel thresh="25000"`, source `#0947B2`) is painted blue on the navy circle, so the circle looks empty.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-blip-duotone-bilevel-not-applied/evidence-2.png)

**3. cisco-cloud-security/11** The Dropbox mark inside the cloud (`biLevel thresh="25000"`, source `#007DE4`) drawn blue-on-blue. The reference thresholds it to white.

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-blip-duotone-bilevel-not-applied/evidence-3.png)

**4. cisco-cloud-security/07** The elastica logo (`biLevel thresh="50000"`) drawn cyan on the cyan band rather than black. The scattering and wrong scale of the wordmark beside it belong to `picture-srcrect-crop-ignored`, not here.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-blip-duotone-bilevel-not-applied/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 7, 11

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(1, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**Confirmed: `<a:blip>`'s effect children are never parsed by `pptx-parse`.**

- `parse_picture` ([`crates/pptx-parse/src/drawing.rs:159`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L159)) reads `<p:blipFill>` at
  [`crates/pptx-parse/src/drawing.rs:167`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L167) and takes exactly two things from it: the
  `r:embed` attribute on `<a:blip>` ([`crates/pptx-parse/src/drawing.rs:169`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L169)) and
  `<a:srcRect>` ([`crates/pptx-parse/src/drawing.rs:186`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L186)). Every element child of
  `<a:blip>` is discarded.
- `Picture` ([`crates/pptx-parse/src/model.rs:211`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L211)) therefore has no field that could hold
  one — `base`, `relationship_id`, `media_part_path`, `crop`, `fill`, `outline`.
- The shape-fill path is the same: `parse_fill` collapses a `<a:blipFill>` to
  `ShapeFill::named("picture")` at [`crates/pptx-parse/src/drawing.rs:579`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L579), keeping neither
  the relationship nor the effects.
- Grepping `crates/` for `duotone`, `biLevel` and `bi_level` returns hits only in
  [`crates/docx-parse/src/image.rs:535`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/image.rs#L535) and [`crates/docx-parse/src/image.rs:552`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/image.rs#L552). There is
  no pptx occurrence at all, in any crate, in any layer.

Because the value never enters the model, the downstream layers are not at fault, but none
of them has a slot to receive a fix either:

- `Primitive::Image` ([`crates/pptx-render/src/display_list.rs:99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L99)) is
  `{object_id, shape_id, name, x, y, w, h, asset_id, stroke, transform}`.
- Both layout arms build that struct from `media_part_path` and the outline only:
  `ShapeKind::Picture` at [`crates/pptx-render/src/layout.rs:425`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L425) (slide shapes, via the
  edit snapshot) and `ShapeNode::Picture` at [`crates/pptx-render/src/layout.rs:534`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L534)
  (master/layout shapes). The host-composed path repeats it at
  [`crates/pptx-render/src/lib.rs:200`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L200).
- The edit snapshot drops it a step earlier still: the picture arm at
  [`crates/pptx-edit/src/deck.rs:139`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L139) seeds only `kind`, `geometry`, `fillJson`,
  `outlineJson` and `mediaPartPath`, and `ShapeSnapshot`
  ([`crates/pptx-edit/src/model.rs:99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L99)) is read back at [`crates/pptx-edit/src/deck.rs:823`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L823)
  with no effects field.
- `paint_image` ([`crates/pptx-raster/src/lib.rs:391`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L391)) decodes at
  [`crates/pptx-raster/src/lib.rs:404`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L404) and blits the pixmap unmodified at
  [`crates/pptx-raster/src/lib.rs:414`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L414). The canvas backend does the same with a bare
  `ctx.drawImage` at [`packages/pptx/src/render/canvas.ts:208`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L208), and `ImagePrimitive`
  ([`packages/pptx/src/types.ts:246`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L246)) has no effects field either.

**Confirmed: the disappearances are contrast collapse, not a skipped draw.** `paint_image`
has exactly one path that skips an image — `None => self.skipped_images += 1` when
`decode` ([`crates/pptx-raster/src/lib.rs:498`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L498)) fails — and that path is unreachable for
these three, because the sources decode fine and are small:

| picture | source | size / mode | mean opaque RGB |
|---|---|---|---|
| cisco/11 OneDrive (id 197, rId10) | `ppt/media/image14.png` | 1983x625 RGBA | `(9, 74, 178)` |
| cisco/11 Dropbox (id 230, rId8) | `ppt/media/image44.png` | 135x126 RGBA | `(0, 126, 229)` |
| cisco/07 elastica (id 106/466, rId20) | `ppt/media/image34.png` | 250x250 RGBA | `(3, 167, 223)` |

Running the ECMA-376 rule for `a:biLevel` (luma below `thresh` → black, at or above →
white) over those means reproduces LibreOffice exactly, which is the strongest available
confirmation that `biLevel` is the only missing step: OneDrive 26.0 % >= 25 % → white,
Dropbox 39.2 % >= 25 % → white, elastica 48.7 % < 50 % → black. The candidate crops in
`evidence-3.png` and `evidence-4.png` show the untransformed source colour sitting in that
exact frame, not an empty frame.

Two scope notes:

- **The cluster under-counts the affected XML.** Across the harness decks there are 9
  `biLevel`, 7 `duotone`, 5 `clrChange` and 9 `lum` blip children in
  `cisco-cloud-security` (slides 02, 03, 07, 11, 23 and layout 22) plus 5 more `biLevel`
  in `project20` slide layouts. The three findings are only the ones a comparator caught;
  a fix should expect to move slides 03 and 23 too.
- `clrChange` (5 occurrences, always paired with a `duotone` or `biLevel` on the same blip
  in this deck) is the effect that actually *can* make artwork vanish, since
  `clrFrom="FFFFFF"` → `clrTo="FFFFFF" alpha=0` knocks a colour out to transparent. It is
  equally unparsed. Whether any of the three findings here is additionally affected by it
  is **not confirmed** — none of the three blips cited above carries a `clrChange`.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Reuse the docx model rather than inventing a pptx one. `ImageEffect` already lives in the
shared crate ([`crates/ooxml-drawingml/src/picture.rs:35`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/picture.rs#L35), re-exported at
[`crates/ooxml-drawingml/src/lib.rs:13`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/lib.rs#L13)) and [`crates/docx-parse/src/image.rs:525`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/image.rs#L525) is a
working `<a:blip>`-child reader for it. The work is to point pptx at the same type, thread
it through the four layers that have no slot for it, and — unlike docx, whose raster
backend refuses images carrying effects at [`crates/docx-raster/src/lib.rs:1133`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-raster/src/lib.rs#L1133) — actually
apply it in `pptx-raster`.

1. **Parse.** Give `Picture` ([`crates/pptx-parse/src/model.rs:211`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L211)) an
   `effects: Vec<ImageEffect>` and fill it in `parse_picture`
   ([`crates/pptx-parse/src/drawing.rs:159`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L159)) from the `<a:blip>` children, in document
   order — order matters, `clrChange` then `duotone` is not `duotone` then `clrChange`.
   Lift `parse_blip_effects` into `ooxml-drawingml` so both formats share one reader, or
   mirror it if the two `XmlElement` types differ. `duotone` and `clrChange` need their
   colours resolved, which docx never did: `ImageEffect::colors` is the field for it, and
   `parse_color_container` ([`crates/pptx-parse/src/drawing.rs:654`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L654)) already handles
   `srgbClr`/`schemeClr`/`sysClr`/`prstClr` with `shade`/`tint`/`satMod` — but it returns
   the *first* colour child, so `duotone`'s two children need iterating, not one call.
   Resolution to hex needs the theme, which the parser does not have; either store two
   `ColorValue`s and resolve in layout the way [`crates/pptx-render/src/layout.rs:1926`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1926)
   does, or pass the theme down. Storing `ColorValue` is the smaller change.

2. **Snapshot.** Add `effectsJson` beside `fillJson` in the picture arm at
   [`crates/pptx-edit/src/deck.rs:139`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L139), a field on `ShapeSnapshot`
   ([`crates/pptx-edit/src/model.rs:99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L99)), and the read-back at
   [`crates/pptx-edit/src/deck.rs:823`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L823). Without this the slide path
   ([`crates/pptx-render/src/layout.rs:425`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L425)) still sees nothing — master and layout
   pictures reach the other arm, so skipping this fixes only half the deck.

3. **Contract.** Add an optional `effects` to `Primitive::Image`
   ([`crates/pptx-render/src/display_list.rs:99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L99)), skip-serialized so a plain picture keeps
   today's JSON and `CONTRACT_VERSION` can stay at 1; mirror it on `ImagePrimitive`
   ([`packages/pptx/src/types.ts:246`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L246)), reusing the `ImageEffect` shape already declared at
   [`packages/docx/src/types/content/image.ts:108`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/docx/src/types/content/image.ts#L108). Populate it in all three producers:
   [`crates/pptx-render/src/layout.rs:425`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L425), [`crates/pptx-render/src/layout.rs:534`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L534) and
   [`crates/pptx-render/src/lib.rs:200`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L200).

4. **Raster.** `ImageCache` ([`crates/pptx-raster/src/lib.rs:723`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L723)) is a budget counter, not
   a map — `decode` re-decodes on every call — so the transform can mutate the returned
   `Pixmap` in place in `paint_image` ([`crates/pptx-raster/src/lib.rs:391`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L391)) with no cache
   key to worry about. The one trap is that `decode` premultiplies before returning
   ([`crates/pptx-raster/src/lib.rs:758`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L758)), so a colour transform must unpremultiply, apply,
   and premultiply again, or be applied inside `decode` before that loop. Applying it
   before the premultiply loop is cleaner and one pass cheaper.

5. **Canvas.** [`packages/docx/src/layout/render/canvasBackend.ts:1406`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/docx/src/layout/render/canvasBackend.ts#L1406) is the reference:
   `biLevel` becomes `grayscale(1) contrast(N)`. Port that switch into `paintImage`
   ([`packages/pptx/src/render/canvas.ts:208`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L208)) via `ctx.filter`. It approximates rather than
   thresholds, and it cannot express `duotone` or `clrChange` at all — an SVG
   `feComponentTransfer` + `feColorMatrix` filter, or an offscreen per-pixel pass, is the
   honest option there. Approximating in canvas while the raster path is exact means the
   two backends disagree on these pictures; decide that deliberately.

```rust
// crates/pptx-raster/src/lib.rs, applied to the RGBA buffer before premultiplying
fn apply_effects(pixels: &mut [[u8; 4]], effects: &[ResolvedEffect]) {
    for effect in effects {
        match effect {
            // luma < thresh -> black, else white; alpha untouched
            ResolvedEffect::BiLevel { threshold } => {
                for p in pixels.iter_mut() {
                    let luma = 0.299 * f32::from(p[0])
                        + 0.587 * f32::from(p[1])
                        + 0.114 * f32::from(p[2]);
                    let v = if luma < threshold * 255.0 { 0 } else { 255 };
                    (p[0], p[1], p[2]) = (v, v, v);
                }
            }
            // luma lerps between the shadow and highlight colours
            ResolvedEffect::Duotone { shadow, highlight } => {
                for p in pixels.iter_mut() {
                    let t = (0.299 * f32::from(p[0])
                        + 0.587 * f32::from(p[1])
                        + 0.114 * f32::from(p[2])) / 255.0;
                    for c in 0..3 {
                        p[c] = (f32::from(shadow[c]) * (1.0 - t)
                            + f32::from(highlight[c]) * t) as u8;
                    }
                }
            }
            ResolvedEffect::Grayscale => { /* ... */ }
        }
    }
}
```

```rust
// crates/pptx-parse/src/drawing.rs, in parse_picture
effects: blip_fill
    .and_then(|value| value.child("blip"))
    .map(parse_blip_effects)
    .unwrap_or_default(),
```

Risks and tests to add:

- **`clrChange` can erase artwork.** It is the one effect in this family that changes
  alpha, and `clrFrom="FFFFFF"` → `clrTo` with `alpha="0"` is exactly the pattern in this
  deck (5 occurrences, `crates/pptx-parse` parses none). Ship it in the same pass as
  `duotone`/`biLevel` or the pictures that pair the two will get half a treatment and look
  worse than they do today. Match on RGB with a tolerance; an exact-equality match on
  antialiased edges leaves a fringe.
- **Premultiplied alpha.** Transforming premultiplied bytes silently darkens semi-
  transparent edges. Every one of these logos is transparent-background PNG, so the bug
  would show as a halo on exactly the pictures this issue is about.
- **Effect order.** `<a:blip>` children are an ordered sequence; applying them as an
  unordered set changes the result whenever `clrChange` and `duotone` co-occur, which is
  4 of the 5 `clrChange` sites here.
- **`duotone` colour resolution needs the theme.** `schemeClr val="bg2"` with `shade` and
  `satMod` is the common form; resolving it in the parser (which has no theme) rather than
  in layout would silently produce black.
- **Two backends diverging.** The canvas filter approximation and an exact raster
  transform will not match pixel for pixel; the golden tests only cover raster, so the gap
  will not be caught automatically.
- Tests to add: a `pptx-parse` unit test for a `<a:blip>` carrying `biLevel`, `duotone`
  and `clrChange` in order; a layout assertion at [`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2008)
  that both picture arms emit the effects; `biLevel` and `duotone` goldens beside
  `golden_image` ([`crates/pptx-raster/tests/golden.rs:283`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/tests/golden.rs#L283)); a premultiply round-trip
  assertion on a semi-transparent source.

**How to verify**

Re-render slides 02, 03, 07, 11 and 23 with
`.venv/bin/python render-improvement-harness/scripts/render_bo.py cisco-cloud-security`
then `diff.py cisco-cloud-security`. These are small glyphs, so the pixel-diff movement is
modest — slide 11's `diff_pct` (10.28) and slide 02's (8.54) should each drop by well under
a point, and slide 07 (41.13) is dominated by other clusters. Check the crops in this
folder rather than the headline number: the four circles in `evidence-2.png` should become
uniform white-on-navy, and the elastica mark in `evidence-4.png` should be black. Note
that `evidence-4.png` will only look fully correct once `picture-srcrect-crop-ignored`
also lands, since the same picture is mis-cropped.

There is no existing coverage to lean on. `crates/pptx-parse` has no test naming any blip
child, and `golden_image` ([`crates/pptx-raster/tests/golden.rs:283`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/tests/golden.rs#L283)) is the only picture
golden — it paints an unmodified checker. New tests belong in three places: a `pptx-parse`
unit test that a `<a:blip>` with `duotone`/`biLevel` populates the new model field, a
layout assertion in [`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2008) that `Primitive::Image` carries
it on both arms, and a `biLevel` and a `duotone` golden beside `golden_image`.
[`crates/pptx-raster/README.md:49`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/README.md#L49) lists the picture-crop gap and should gain a line for
effects.

**Additional context**

none.

Related issues found in the same run: `picture-srcrect-crop-ignored`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/ooxml-drawingml/src/picture.rs`, `crates/pptx-edit/src/deck.rs`, `crates/pptx-edit/src/model.rs`, `crates/pptx-render/src/display_list.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/lib.rs`, `crates/pptx-raster/src/lib.rs`, `crates/pptx-raster/README.md`, `packages/pptx/src/types.ts`, `packages/pptx/src/render/canvas.ts`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/picture-blip-duotone-bilevel-not-applied/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
