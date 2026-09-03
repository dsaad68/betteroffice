---
id: picture-srcrect-crop-ignored
title: Apply a picture's srcRect crop and its spPr preset-geometry mask
category: picture
impact: high
effort: medium
confidence: high
status: open
occurrences: 14
decks: [cisco-cloud-security, project17]
findings: [cisco-cloud-security/08/2, cisco-cloud-security/08/5, cisco-cloud-security/12/4, cisco-cloud-security/15/4, cisco-cloud-security/17/3, cisco-cloud-security/18/3, cisco-cloud-security/23/4, project17/02/3, project17/04/5, project17/06/5, project17/07/5, project17/10/4, project17/10/6, project17/12/5]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/pptx-edit/src/deck.rs, crates/pptx-edit/src/model.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/lib.rs, crates/pptx-render/src/display_list.rs, crates/pptx-raster/src/lib.rs, crates/pptx-raster/README.md, packages/pptx/src/types.ts, packages/pptx/src/render/canvas.ts]
---

## Symptom

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

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/08 | Picture 8 (`srcRect t="251" b="16720"`): the OS X dock the bottom 16.7% crop should remove is painted inside the laptop screen. |
| 2 | cisco-cloud-security/17 | Picture 2 (`l="6360" r="6360" b="8637"`) and Picture 8 (`t="251" b="16720"`): the laptop's black bezel is replaced by the source's transparent margin, and the dock strip bleeds into the screen. |
| 3 | project17/04 | Slide-master Picture 11 (`l="15358" t="9779" r="21228" b="24405"`): the tagline row under the wordmark is visible and the mark itself renders smaller. |
| 4 | project17/10 | Pictures 6 and 58 carry `<a:prstGeom prst="ellipse">`; both paint as square, uncropped rectangles. |

## Root cause (hypothesis)

**Confirmed for `srcRect`.** The value is parsed and then dropped one layer later.

- `crates/pptx-parse/src/drawing.rs:186` reads `<a:srcRect>` into `Picture::crop`, and
  `crates/pptx-parse/src/drawing.rs:752` fills every side of `PictureCrop`
  (field at `crates/pptx-parse/src/model.rs:216`, type at
  `crates/pptx-parse/src/model.rs:222`) in OOXML thousandths of a percent.
- Nothing downstream reads that field. `crates/pptx-raster/README.md:49` already records
  this: "`PictureCrop` (`srcRect`) is parsed but dropped by the layout pass".
- Both layout paths drop it. Master and layout shapes go through `render_parsed_shape`,
  whose `ShapeNode::Picture` arm at `crates/pptx-render/src/layout.rs:534` builds
  `Primitive::Image` from `media_part_path`, `outline` and the rect only — this is the
  project17 master-logo case. Slide shapes go through `render_snapshot_shape`, whose
  `ShapeKind::Picture` arm at `crates/pptx-render/src/layout.rs:425` does the same — this
  is the cisco case.
- The snapshot path loses it even earlier: `crates/pptx-edit/src/deck.rs:139` seeds a
  picture into the collaborative document with only `kind`, `geometry`, `fillJson`,
  `outlineJson` and `mediaPartPath`, and `ShapeSnapshot`
  (`crates/pptx-edit/src/model.rs:99`) has no crop field, so
  `crates/pptx-edit/src/deck.rs:823` cannot read one back.
- The display-list contract has no place to put it either: `Primitive::Image` at
  `crates/pptx-render/src/display_list.rs:99` is `{x, y, w, h, asset_id, stroke,
  transform}`. The host-composed path repeats the omission at
  `crates/pptx-render/src/lib.rs:48` and `crates/pptx-render/src/lib.rs:200`.
- Consequently both backends stretch: `paint_image` at
  `crates/pptx-raster/src/lib.rs:391` builds `fit` from `frame.width() / source.width()`
  and `frame.height() / source.height()` (`crates/pptx-raster/src/lib.rs:406`) and blits
  the whole pixmap; the canvas backend calls the 5-argument
  `ctx.drawImage(source, x, y, w, h)` at `packages/pptx/src/render/canvas.ts:208`, with
  `ImagePrimitive` (`packages/pptx/src/types.ts:246`) carrying no crop.

For contrast, docx already models this end to end: `RelativeRect` at
`crates/docx-parse/src/shape.rs:68`, `pictureSrcRect` in the display list at
`crates/docx-layout/src/display_list.rs:7613`, and `drawCroppedImage` at
`packages/docx/src/layout/render/canvasBackend.ts:1340`.

**Confirmed for the geometry mask, and it is a deeper gap: the value is never parsed at
all.** `parse_picture` (`crates/pptx-parse/src/drawing.rs:158`) reads `spPr` only for
`xfrm`, `parse_fill` and `parse_outline` (`crates/pptx-parse/src/drawing.rs:181-188`); it
never touches `<a:prstGeom>`, and `Picture` (`crates/pptx-parse/src/model.rs:210`) has no
`geometry` or `adjust_values` field. The edit snapshot then hardcodes
`shape_map.insert(txn, "geometry", "rect")` for pictures at
`crates/pptx-edit/src/deck.rs:141`. So a picture's preset shape is lost at the parser, not
at the renderer.

Verified against the decks' XML: `project17/xml/10/slide.xml` has
`<a:prstGeom prst="ellipse">` on Picture 6 and Picture 58, and
`cisco-cloud-security/xml/17/slide.xml` has `<a:prstGeom prst="roundRect">` on Picture
108, so this is not ellipse-only.

Two smaller points, both flagged as hypotheses:

- `integer_attribute` (`crates/pptx-parse/src/drawing.rs:942`) parses into `i32`, so a
  negative `srcRect` (an outset, legal in `ST_Percentage`) survives parsing. Whatever
  consumes the crop has to clamp or letterbox rather than index outside the source.
- Some project17 pictures also sit a few pixels off horizontally from the reference
  (visible in `evidence-4.png`). That offset is not explained by the crop and is probably
  a separate transform issue; not investigated here.

## Verification

Re-render `cisco-cloud-security` slides 08, 12, 15, 17, 18, 23 and `project17` slides 02,
04, 06, 07, 10, 12 with
`.venv/bin/python render-improvement-harness/scripts/render_bo.py <deck>` followed by
`diff.py <deck>`. The cisco laptop-mockup slides (08, 17, 18) carry most of the pixel cost
of this cluster; their `fine_pct` should drop noticeably once the dock and bezel strips
disappear. `project17` slides move less — the master logo is a small corner shape — but
slide 10's two portraits should become circles.

Existing coverage to extend: `golden_image` at `crates/pptx-raster/tests/golden.rs:283` is
the only picture golden and it paints an uncropped checker; add a cropped and a masked
variant beside it. `crates/pptx-parse` has no `srcRect` test at all — the only occurrence
of that string under `crates/` outside docx is the parser line itself — so a parse test
for `PictureCrop` and for a picture's `prstGeom` is new ground.
`crates/pptx-render/src/layout.rs:2008` holds the layout unit tests, where an assertion
that the emitted `Primitive::Image` carries the crop belongs.
