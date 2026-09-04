---
id: transform-group-child-rotation-scale-wrong
title: Fold into geometry-custom-collapses-to-bbox — the group child transform is already correct
category: transform
impact: low
effort: easy
confidence: high
status: duplicate
occurrences: 1
decks: [swot-analysis]
findings: [swot-analysis/01/1]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-render/src/layout.rs]
---

## Symptom

On `swot-analysis/01` the small shading crescent that hugs the rim of each SWOT icon renders as an
oversized diamond that pokes out past the circle, on all four icons (evidence-1.png,
evidence-2.png). The finding attributes this to the group's anisotropic `chOff`/`chExt` scale being
composed wrongly with the child's own `rot="18900000"`.

**That attribution is wrong.** Measured against the rendered pixels, BetterOffice's group child
transform is correct to within ~2px; the diamond is simply the *bounding rectangle* of the
`a:custGeom` freeform, correctly scaled and correctly rotated 315° (evidence-3.png). This is
`geometry-custom-collapses-to-bbox` seen through a 315° rotation, nothing more.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | swot-analysis/01 | The "S" icon at 2x: reference draws a thin rim-hugging crescent, candidate draws a straight-edged diamond — a rotated rectangle, not a mis-scaled freeform. |
| 2 | swot-analysis/01 | All four icons; the same single shape repeated four times in four sibling groups, so the cluster's one finding is really one shape. |
| 3 | swot-analysis/01 | The proof. Green = the freeform's real path, transformed by BetterOffice's own model, drawn over the reference: it traces the reference crescent exactly. Red = the same freeform's *bounding box* under the same transform, drawn over the candidate: it traces the candidate diamond exactly. |

## Root cause (hypothesis)

**Confirmed, and it is not a transform bug.** The transform hypothesis in the finding is
**not confirmed — it is refuted by measurement.**

### What the code does

`Space::for_group` (`crates/pptx-render/src/layout.rs:1622`) builds an axis-aligned
scale-plus-translate from the group's `a:xfrm`: `scale_x = rect.w / chExt.cx`,
`scale_y = rect.h / chExt.cy` (`crates/pptx-render/src/layout.rs:1628-1629`), origin shifted by
`chOff` (`crates/pptx-render/src/layout.rs:1633-1634`). `Space::map_transform`
(`crates/pptx-render/src/layout.rs:1613`) maps a child's **unrotated** `a:off`/`a:ext` through it.
The child's own `rot` never touches that box: it is carried separately onto the primitive as
`Transform { rotation_deg, .. }` (`crates/pptx-render/src/layout.rs:392-396` on the snapshot path,
`crates/pptx-render/src/layout.rs:500-504` on the parsed path) and applied by the raster backend as
a rotation about the primitive box's centre (`crates/pptx-raster/src/lib.rs:556-565`, composed at
`crates/pptx-raster/src/lib.rs:241`).

That is exactly PowerPoint's model — scale the unrotated box by the group factors, then rotate the
result about its own centre; the anisotropic scale never shears the rotated shape.

### The measurement

For `Freeform: Shape 21` (id 22) in `Group 24` (id 25), with `sx = 2351315/4063689 = 0.578625`,
`sy = 2104570/3744686 = 0.562014`, child `off (4464751, 2391876)` `ext (3387166, 2336802)`,
`rot="18900000"` (315°), at 1280x720:

| corner | predicted (px) | measured in `bo-img/01.png` |
|---|---|---|
| right | (658.4, 240.8) | (657, 240) |
| top | (560.9, 143.3) | (560, 143) |
| bottom | (512.9, 386.3) | (512.5, 384) |

Rasterising the freeform's real `a:pathLst` through the *same* transform and comparing against the
reference: **100.0% of the reference crescent's pixels fall inside the predicted path** on the S, O
and W icons and 99.8% on the T icon. Everything inside the predicted path that the reference does
not paint in the crescent colour is accounted for — 15929px covered by the later `Oval 23`
highlight, 1253px by the white "S" glyph, the rest antialiasing. Conversely **100.0% of the
candidate's diamond pixels fall inside the freeform's bounding box under that same transform**, on
all four icons.

So both engines place the shape identically. The reference draws the authored outline; the
candidate draws its bounding rectangle.

### Why the rectangle

Same chain as `geometry-custom-collapses-to-bbox`: `parse_geometry`
(`crates/pptx-parse/src/drawing.rs:335`) reduces `<a:custGeom>` to the string `"custom"`
(`crates/pptx-parse/src/drawing.rs:340-343`) and never reads `a:pathLst`; `geometry_path`
(`crates/pptx-render/src/layout.rs:1946`) asks `preset_geometry_to_path` for `"custom"`, gets `None`
(`crates/ooxml-drawingml/src/geometry.rs:227`), and falls back to `"rect"`
(`crates/pptx-render/src/layout.rs:1955-1956`).

The path here is a circle-segment cap — flat chord from `(314725,0)` to `(3072440,0)`, then two
`cubicBezTo` arcs down to `(1693583,2336802)` — occupying only ~40% of its 3387166x2336802 box. Fill
in the path and the diamond becomes the crescent.

### What this cluster adds over `geometry-custom-collapses-to-bbox`

- It is the corpus's **only** custGeom shape under a non-uniformly scaled group *and* a non-zero
  child rotation, which makes it the sharpest regression fixture for that fix: a wrong compose order
  (rotate-then-scale, i.e. shearing) would move the corners by tens of pixels and the measurement
  above would catch it. The 17 findings in the other cluster are all axis-aligned or uniformly
  scaled.
- It independently pins the transform contract described above, which the other report does not
  touch.

### Latent gap found while checking, *not* this finding's cause

`Space` (`crates/pptx-render/src/layout.rs:1596-1601`) carries only origin and scale, so a group's
own `rot`/`flipH`/`flipV` on `<p:grpSpPr><a:xfrm>` is dropped for its children at both group sites
(`crates/pptx-render/src/layout.rs:367-368`, `crates/pptx-render/src/layout.rs:492`). All four
groups on this slide have no `rot`, so it does not affect this cluster. Across the whole corpus only
2 of 929 group transforms carry `rot` (`cisco-cloud-security/07` `rot="20679101"`,
`cisco-cloud-security/09` `rot="476079"`) and neither produced a filed finding — **not confirmed as
a visible defect**, and out of scope here.

## Verification

Nothing to fix independently. Fold `swot-analysis/01/1` into `geometry-custom-collapses-to-bbox` and
add `swot-analysis` to that cluster's verification set.

After the custGeom path parse lands, re-render with
`.venv/bin/python render-improvement-harness/scripts/render_bo.py swot-analysis` then
`diff.py swot-analysis`. Slide 01's `diff_pct` is 8.86; the four diamonds are the only geometric
difference on the slide (findings 2 and 3 are `lo-suspect` text wraps where LibreOffice is the wrong
one), so it should drop to roughly the text-wrap residue. Check the crescents sit *inside* the
circle rims and that the freeform's straight chord edge runs bottom-left to top-right at 45°, which
is what proves the rotation survived the fix.

Worth pinning as a test: no test under `crates/pptx-render` covers a rotated child inside an
anisotropically scaled group. `Space::for_group` has no direct unit test — the nearest coverage is
the emitted-primitive assertions around `crates/pptx-render/src/layout.rs:2152-2154`, and the raster
side's box-to-pixel contract at `crates/pptx-raster/src/lib.rs:953`
(`geometry_commands_scale_by_the_primitive_box`). A layout test asserting the four rotated corners
of this exact shape would lock in the no-shear compose order.
