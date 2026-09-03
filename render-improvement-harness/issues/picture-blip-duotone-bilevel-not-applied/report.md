---
id: picture-blip-duotone-bilevel-not-applied
title: Parse and apply the blip colour effects (duotone, biLevel, clrChange, lum)
category: picture
impact: low
effort: medium
confidence: high
status: open
occurrences: 3
decks: [cisco-cloud-security]
findings: [cisco-cloud-security/02/3, cisco-cloud-security/07/6, cisco-cloud-security/11/3]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/ooxml-drawingml/src/picture.rs, crates/pptx-edit/src/deck.rs, crates/pptx-edit/src/model.rs, crates/pptx-render/src/display_list.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/lib.rs, crates/pptx-raster/src/lib.rs, crates/pptx-raster/README.md, packages/pptx/src/types.ts, packages/pptx/src/render/canvas.ts]
---

## Symptom

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

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/02 | Picture 10 (`biLevel thresh="25000"`, Yammer) renders teal instead of white, and Picture 6 (`duotone` bg2→white, Google Drive) renders full-colour instead of grey-to-white. |
| 2 | cisco-cloud-security/11 | The app-logo row: Google Drive keeps its brand colours, and the OneDrive mark (`biLevel thresh="25000"`, source `#0947B2`) is painted blue on the navy circle, so the circle looks empty. |
| 3 | cisco-cloud-security/11 | The Dropbox mark inside the cloud (`biLevel thresh="25000"`, source `#007DE4`) drawn blue-on-blue. The reference thresholds it to white. |
| 4 | cisco-cloud-security/07 | The elastica logo (`biLevel thresh="50000"`) drawn cyan on the cyan band rather than black. The scattering and wrong scale of the wordmark beside it belong to `picture-srcrect-crop-ignored`, not here. |

## Root cause (hypothesis)

**Confirmed: `<a:blip>`'s effect children are never parsed by `pptx-parse`.**

- `parse_picture` (`crates/pptx-parse/src/drawing.rs:159`) reads `<p:blipFill>` at
  `crates/pptx-parse/src/drawing.rs:167` and takes exactly two things from it: the
  `r:embed` attribute on `<a:blip>` (`crates/pptx-parse/src/drawing.rs:169`) and
  `<a:srcRect>` (`crates/pptx-parse/src/drawing.rs:186`). Every element child of
  `<a:blip>` is discarded.
- `Picture` (`crates/pptx-parse/src/model.rs:211`) therefore has no field that could hold
  one — `base`, `relationship_id`, `media_part_path`, `crop`, `fill`, `outline`.
- The shape-fill path is the same: `parse_fill` collapses a `<a:blipFill>` to
  `ShapeFill::named("picture")` at `crates/pptx-parse/src/drawing.rs:579`, keeping neither
  the relationship nor the effects.
- Grepping `crates/` for `duotone`, `biLevel` and `bi_level` returns hits only in
  `crates/docx-parse/src/image.rs:535` and `crates/docx-parse/src/image.rs:552`. There is
  no pptx occurrence at all, in any crate, in any layer.

Because the value never enters the model, the downstream layers are not at fault, but none
of them has a slot to receive a fix either:

- `Primitive::Image` (`crates/pptx-render/src/display_list.rs:99`) is
  `{object_id, shape_id, name, x, y, w, h, asset_id, stroke, transform}`.
- Both layout arms build that struct from `media_part_path` and the outline only:
  `ShapeKind::Picture` at `crates/pptx-render/src/layout.rs:425` (slide shapes, via the
  edit snapshot) and `ShapeNode::Picture` at `crates/pptx-render/src/layout.rs:534`
  (master/layout shapes). The host-composed path repeats it at
  `crates/pptx-render/src/lib.rs:200`.
- The edit snapshot drops it a step earlier still: the picture arm at
  `crates/pptx-edit/src/deck.rs:139` seeds only `kind`, `geometry`, `fillJson`,
  `outlineJson` and `mediaPartPath`, and `ShapeSnapshot`
  (`crates/pptx-edit/src/model.rs:99`) is read back at `crates/pptx-edit/src/deck.rs:823`
  with no effects field.
- `paint_image` (`crates/pptx-raster/src/lib.rs:391`) decodes at
  `crates/pptx-raster/src/lib.rs:404` and blits the pixmap unmodified at
  `crates/pptx-raster/src/lib.rs:414`. The canvas backend does the same with a bare
  `ctx.drawImage` at `packages/pptx/src/render/canvas.ts:208`, and `ImagePrimitive`
  (`packages/pptx/src/types.ts:246`) has no effects field either.

**Confirmed: the disappearances are contrast collapse, not a skipped draw.** `paint_image`
has exactly one path that skips an image — `None => self.skipped_images += 1` when
`decode` (`crates/pptx-raster/src/lib.rs:498`) fails — and that path is unreachable for
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

## Verification

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
child, and `golden_image` (`crates/pptx-raster/tests/golden.rs:283`) is the only picture
golden — it paints an unmodified checker. New tests belong in three places: a `pptx-parse`
unit test that a `<a:blip>` with `duotone`/`biLevel` populates the new model field, a
layout assertion in `crates/pptx-render/src/layout.rs:2008` that `Primitive::Image` carries
it on both arms, and a `biLevel` and a `duotone` golden beside `golden_image`.
`crates/pptx-raster/README.md:49` lists the picture-crop gap and should gain a line for
effects.
