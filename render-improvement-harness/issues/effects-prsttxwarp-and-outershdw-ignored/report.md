---
id: effects-prsttxwarp-and-outershdw-ignored
title: Parse a:prstTxWarp and a:effectLst/a:outerShdw and render them
category: effects
impact: low
effort: hard
confidence: high
status: open
occurrences: 3
decks: [ocp-psp-plan, project20]
findings: [ocp-psp-plan/01/2, ocp-psp-plan/03/3, project20/04/6]
files: [crates/pptx-parse/src/model.rs, crates/pptx-parse/src/drawing.rs, crates/pptx-render/src/display_list.rs, crates/pptx-render/src/layout.rs, crates/pptx-raster/src/lib.rs, crates/pptx-raster/src/font.rs, crates/pptx-edit/src/model.rs, crates/pptx-edit/src/deck.rs, crates/ooxml-drawingml/src/geometry.rs, packages/pptx/src/types.ts, packages/pptx/src/render/canvas.ts]
---

## Symptom

Two unrelated DrawingML properties share one root cause: neither is read out of the XML at all.

`a:bodyPr/a:prstTxWarp` with `prst="textArchUp"` / `"textArchDown"` is a WordArt-style
text-on-a-path transform. The candidate lays the run out as ordinary straight text inside the
shape rectangle and applies only the shape's own `rot`. On ocp-psp-plan/01 the un-warped
`INCENTIVES` label lands flat across the middle of the white centre circle instead of curving
along the bottom cyan band, and `COMMUNITIES` is cut to `COMMUNITI` (evidence-1.png). On
ocp-psp-plan/03 both ring labels become straight diagonals cutting across the panel
(evidence-2.png).

`a:effectLst/a:outerShdw` is a drop shadow. The candidate draws no shadow of any kind: the
project20/04 status diamonds render as flat, hard-edged shapes where the reference has a soft
offset halo (evidence-3.png).

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | ocp-psp-plan/01 | `textArchDown` / `textArchUp` on the three ring labels. Reference curves `COMMUNITIES`, `GROWTH` and `INCENTIVES` around the donut; candidate draws `INCENTIVES` flat inside the centre circle and clips `COMMUNITIES` to `COMMUNITI` |
| 2 | ocp-psp-plan/03 | `Powering Partner GROWTH` (`textArchUp`, rot 72.6 deg) and `Deepening COMMUNITY Connection` (`textArchDown`, rot 300.1 deg). Reference arcs them along the navy ring; candidate draws two straight rotated lines |
| 3 | project20/04 | the two status diamonds at 6x. Reference has the `outerShdw blurRad="25400" dist="25400" dir="2700000"` halo down-right of each; candidate has nothing. (The missing green dashed connector in the same crop is a separate issue, `unsupported-element`.) |

## Root cause (hypothesis)

Confirmed, not a hypothesis: both elements are never parsed, so nothing downstream can act on
them.

**`a:prstTxWarp`.** `parse_text_body` reads exactly five things off `a:bodyPr` -- `anchor`,
`vert`, the autofit child, and the four insets (`crates/pptx-parse/src/drawing.rs:764-788`).
`prstTxWarp` is not among them, and `TextBody` has no field that could hold it
(`crates/pptx-parse/src/model.rs:269-278`). The display list has no representation for warped
text either: a `Primitive::TextBox` carries `PositionedTextLine`s whose runs hold
`PositionedGlyph { glyph_id, cluster, x, advance, x_offset, y_offset }`
(`crates/pptx-render/src/display_list.rs:248-255`) -- a per-glyph pen position on a straight
baseline and no per-glyph rotation. The only rotation the contract can express is the whole-box
`Transform { rotation_deg, flip_h, flip_v }`
(`crates/pptx-render/src/display_list.rs:63-68`), which is what both backends apply:
`pptx-raster` paints every glyph of a line with the same box transform
(`crates/pptx-raster/src/font.rs:46-67` and `crates/pptx-raster/src/font.rs:86-106`), and the
layout pass emits the box with the shape's `rot` only
(`crates/pptx-render/src/layout.rs:742-755`). That is exactly what evidence-2.png shows: the
shape rotation is there, the warp is not.

The `COMMUNITI` truncation in evidence-1.png is a compound effect, not warp code: the
un-warped run is wider than the box (the box was authored to fit the *arc*, which is shorter
across than along), and `pptx-raster` clips a `TextBox` to its own rect before painting
(`crates/pptx-raster/src/lib.rs:282-297`, via `clipped` at
`crates/pptx-raster/src/lib.rs:325-355`). The clip behaviour itself belongs to
`text-overflow-autofit-not-handled`; fixing the warp removes the overflow that triggers it.

**`a:outerShdw`.** `parse_shape` reads `a:xfrm`, `a:prstGeom`/`a:custGeom`, `a:avLst`, the fill
and the outline off `p:spPr` and stops (`crates/pptx-parse/src/drawing.rs:138-156`);
`parse_picture` does the same (`crates/pptx-parse/src/drawing.rs:159-190`). Neither looks at
`a:effectLst`, and neither `Shape` (`crates/pptx-parse/src/model.rs:197-207`) nor `Picture`
(`crates/pptx-parse/src/model.rs:211-220`) nor `ShapeSnapshot` on the edit path
(`crates/pptx-edit/src/model.rs:99-121`) has a field for effects. `Primitive::Shape` and
`Primitive::Image` carry only `fill`, `stroke` and `transform`
(`crates/pptx-render/src/display_list.rs:78-111`), and `paint_shape` fills the path then strokes
it, with no third pass (`crates/pptx-raster/src/lib.rs:364-385`). No code under `crates/`
matches `outerShdw` outside `docx-parse`; the only pptx mention is
`crates/pptx-parse/src/write.rs:1003`, where `effectLst` appears in `POST_FILL_ELEMENTS` purely
so the writer inserts a replacement fill *before* it. Saving therefore round-trips the element
untouched -- this is a render-side gap only, not data loss.

Two dependencies worth naming:

- The shadow colour on every occurrence here is `<a:schemeClr val="bg1"><a:lumMod val="50000"/>
  <a:alpha val="40000"/></a:schemeClr>`. `resolve_color_value_to_rgba_hex`
  (`crates/ooxml-drawingml/src/color.rs:91-102`) already emits `#RRGGBBAA` and `pptx-raster`'s
  `parse_color` already accepts 8-digit hex (`crates/pptx-raster/src/lib.rs:780-799`), so the
  alpha path exists -- but the layout pass must use the rgba resolver for shadows, or every
  shadow paints opaque. Same underlying defect as `fill-alpha-modifier-ignored`.
- `tiny-skia` 0.12 has no blur filter (no `blur` symbol anywhere in its sources), so `blurRad`
  has to be hand-rolled: render the shadow shape into an offscreen `Pixmap` and run a separable
  box blur over it before compositing. The browser backend gets this free from `ctx.shadowBlur`;
  `packages/pptx/src/render/canvas.ts:117-199` has no shadow handling today.

**Scope across the corpus** (grep over `decks/*/xml/*/slide.xml`): non-identity `prstTxWarp`
appears in exactly one deck -- ocp-psp-plan, 23 occurrences. `prst="textNoShape"` is the
identity warp and must stay a no-op; it appears 158 times across ocp-psp-plan and project20 and
must not be treated as a warp. `a:outerShdw` is far more widespread: project17 476,
cisco-cloud-security 19, project20 14, triangles-corporate 4, typography-trick 3,
green-solutions 1. Only project20/04 produced a finding, because the comparator judged the rest
below the reporting bar -- see `decks/project17/reports/02.md:95`, which explicitly parks the
purple circle's shadow as "present and comparable in both renders" (unverified: that shadow is
probably baked into the source bitmap). So the shadow half is low-severity but high-frequency,
and the warp half is the reverse.

**Recommendation:** split this cluster. The two halves share nothing but a taxonomy label -- no
file, no data structure, no test. `effects-outershdw-not-drawn` is a self-contained medium job
that touches 6 of the 12 decks; `effects-prsttxwarp-not-applied` is a hard job for one deck and
should be scheduled behind it. The combined `hard` in the front matter reflects doing both.

## Verification

Re-render `ocp-psp-plan` slides 01 and 03 and `project20` slide 04.

- ocp-psp-plan/03 (33.01% today) is dominated by `geometry-custom-collapses-to-bbox` and
  `fill-alpha-modifier-ignored`; the two warped labels are a few hundred pixels, so expect only
  a fraction of a point from this issue alone. Judge it on evidence-2.png's crop, not the
  slide-level number.
- ocp-psp-plan/01 (12.93%): the three ring labels should land on their arcs and `COMMUNITIES`
  should read in full. Same caveat -- the bounding-box wedges dominate the diff.
- project20/04 (8.89%): the ten diamonds should gain their halo. Sub-0.1% on the slide number;
  verify against evidence-3.png.

Because none of the three slides is diff-limited by this issue, gate the work on unit tests
rather than on the slide diff:

- `crates/pptx-parse/src/drawing.rs:951` tests -- `prstTxWarp prst="textArchUp"` reaches the
  model, `textNoShape` does not, and `outerShdw`'s attributes and colour reach the model.
- `crates/pptx-parse/src/write.rs` -- a round-trip of a deck carrying `a:effectLst` still has it.
- `crates/pptx-render/src/layout.rs:2008` tests -- the `Primitive::Shape` for a shadowed shape
  carries a shadow whose colour is 8-digit hex.
- `crates/pptx-raster/src/lib.rs:803` tests -- a shadowed shape puts non-background pixels
  down-right of its geometry and an unshadowed one does not; glyph ink for an arched run leaves
  the straight baseline band.
- `packages/pptx/src/render/canvas.test.ts` -- the canvas backend sets `shadowColor` and
  `shadowBlur` for a shadowed shape.
