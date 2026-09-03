---
id: geometry-custom-collapses-to-bbox
title: Parse a:custGeom path data and emit it instead of the bounding-box rectangle
category: geometry-custom
impact: high
effort: medium
confidence: high
status: open
occurrences: 17
decks: [cisco-cloud-security, ocp-psp-plan, project17, project20]
findings: [cisco-cloud-security/02/1, cisco-cloud-security/03/1, cisco-cloud-security/04/1, cisco-cloud-security/05/2, cisco-cloud-security/06/1, cisco-cloud-security/07/3, cisco-cloud-security/11/1, cisco-cloud-security/13/2, cisco-cloud-security/16/5, cisco-cloud-security/19/1, cisco-cloud-security/20/1, ocp-psp-plan/01/1, ocp-psp-plan/03/1, project17/04/3, project17/08/3, project17/11/5, project20/16/3]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/lib.rs, crates/pptx-edit/src/deck.rs, crates/pptx-edit/src/model.rs, crates/docx-parse/src/drawingml.rs]
---

## Symptom

Every `<a:custGeom>` shape paints as a plain axis-aligned rectangle spanning its `a:off`/`a:ext`
box, in the shape's own fill and stroke, with none of the authored outline. A donut chart becomes
a pile of overlapping rectangles (`evidence-1.png`); a honeycomb of hexagons becomes a plus sign
of white boxes (`evidence-2.png`); 131 person silhouettes become 131 blue squares
(`evidence-3.png`); a Venn diagram of two traced circles plus a lens and a handshake glyph becomes
two grey slabs and a white square (`evidence-4.png`). The failure does not depend on how the path
is written — literal `lnTo` polygons, `cubicBezTo` wedges, and paths whose shape carries legacy
`T0..Tn` or `connsiteX/Y` guides all collapse the same way.

At 17 findings across 4 decks this is the single largest visual defect in the corpus; on
`cisco-cloud-security/04` and `/19` it drives every top hot cell.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | ocp-psp-plan/01 | 21 `cubicBezTo` donut wedges (`Freeform 6..46`, ids 3-23) render as stacked rectangles; the ring, its label segments and the outer band all disappear. |
| 2 | project17/08 | Seven literal 6-point hexagons (`Freeform 5/6/7/8/9/11/18`, ids 10-20, path `w="856" h="584"`) render as white rectangles, leaving purple gaps where the corners were cut. |
| 3 | cisco-cloud-security/04 | The person-silhouette freeforms inside every node group (`Freeform 994` id 6 and 130 siblings) render as filled squares inside their correct circles. |
| 4 | project17/04 | `Freeform 58`/`59` (two ~150-point traced circles) become grey slabs, `Freeform 57` (the lens) is not visible at all, and `Freeform 16` (the bezier handshake) becomes a white box. |

## Root cause (hypothesis)

**Confirmed. The path data is never parsed; the renderer then substitutes a rectangle.**

1. `parse_geometry` (`crates/pptx-parse/src/drawing.rs:335`) is the only code in `crates/pptx-parse`
   that touches `custGeom`. It checks for the element's presence and returns the string `"custom"`
   (`crates/pptx-parse/src/drawing.rs:341-343`); `a:pathLst` is never read. `Shape`
   (`crates/pptx-parse/src/model.rs:198`) accordingly carries only `geometry: String`
   (`crates/pptx-parse/src/model.rs:201`) and `adjust_values`
   (`crates/pptx-parse/src/model.rs:203`), and `parse_adjust_values`
   (`crates/pptx-parse/src/drawing.rs:355-359`) reads guides from `prstGeom/avLst` only — a
   `custGeom`'s `gdLst` is not evaluated either.

2. Every layout path turns that string into a shape by asking the preset table for it.
   `geometry_path` (`crates/pptx-render/src/layout.rs:1946`) calls
   `preset_geometry_to_path(geometry, ...)` and, when that returns `None`, falls back to
   `preset_geometry_to_path("rect", ...)` (`crates/pptx-render/src/layout.rs:1955-1957`).
   `preset_geometry_to_path` (`crates/ooxml-drawingml/src/geometry.rs:38`) has no `"custom"` arm and
   ends in `_ => return None` (`crates/ooxml-drawingml/src/geometry.rs:227`), so the fallback always
   fires. That is the rectangle on screen.

3. All three emit sites share the same helper, which is why no deck escapes it: slide shapes at
   `crates/pptx-render/src/layout.rs:410` (snapshot path), master/layout shapes at
   `crates/pptx-render/src/layout.rs:516` (parsed path), and the host-composed path at
   `crates/pptx-render/src/lib.rs:170` via its own copy of the same fallback at
   `crates/pptx-render/src/lib.rs:237-250`.

4. The edit snapshot cannot carry a path even if one existed. `seed_shape` writes only `geometry`
   and `adjustValuesJson` into the collaborative document (`crates/pptx-edit/src/deck.rs:125-129`),
   and `ShapeSnapshot` (`crates/pptx-edit/src/model.rs:111`) has no path field, so the read-back at
   `crates/pptx-edit/src/deck.rs:816` has nothing to return.

**Nothing downstream needs to change.** The display-list contract already carries arbitrary
geometry: `Primitive::Shape` holds `path: Vec<GeometryPathCommand>`
(`crates/pptx-render/src/display_list.rs:89`), the raster backend scales those unit-space commands
into the primitive box in `geometry_path` (`crates/pptx-raster/src/lib.rs:575-600`, used at
`crates/pptx-raster/src/lib.rs:376`), and the canvas backend does the same in `buildPath`
(`packages/pptx/src/render/canvas.ts:118`, typed at `packages/pptx/src/types.ts:240`). The chart
renderer already exercises exactly this: `PlotOp::Path` emits `geometry: "custom"` with real
commands at `crates/pptx-render/src/chart.rs:155-179`, and both backends draw those pie wedges
correctly. So the gap is confined to the shape parse to layout hand-off.

**Working prior art exists in this repo.** `docx-parse` implements the whole feature:
`parse_custom_geometry_path` (`crates/docx-parse/src/drawingml.rs:276`), the per-path walker
`parse_custom_path` (`crates/docx-parse/src/drawingml.rs:297`) covering
`moveTo`/`lnTo`/`quadBezTo`/`cubicBezTo`/`arcTo`/`close`, `arc_to_cubics`
(`crates/docx-parse/src/drawingml.rs:395`), unit-space normalisation by the path's own `w`/`h` in
`normalize_raw_path` (`crates/docx-parse/src/drawingml.rs:434`), guide seeding in
`build_custom_guides` (`crates/docx-parse/src/drawingml.rs:529`) and the formula evaluator
`evaluate_guide` (`crates/docx-parse/src/drawingml.rs:604`), bounded by
`MAX_CUSTOM_PATH_COMMANDS = 2_048` and `MAX_CUSTOM_GUIDES = 512`
(`crates/docx-parse/src/drawingml.rs:14-15`). It is wired in at
`crates/docx-parse/src/shape.rs:418-420` onto `Shape::geometry_path`
(`crates/docx-parse/src/shape.rs:341`). It lives in `docx-parse`, not in the shared
`ooxml-drawingml`, and is written against `docx_parse::xml::XmlElement`, which is a different type
from `pptx_parse::xml::XmlElement` (`crates/pptx-parse/src/drawing.rs:8`) — so it is a port or a
generalisation, not a re-export. `pptx-parse` does already have its own guide-formula evaluator,
`evaluate_guide_formula` (`crates/pptx-parse/src/drawing.rs:445`), covering `val`, `*/`, `+-`, `+/`,
`?:`, `abs`, `at2`, `cat2`, `cos`, `max`, `min`, `mod`, `pin`, `sat2`, `sin`, `sqrt` — it is only
ever fed `prstGeom/avLst` today.

### What the corpus actually needs (measured)

Scanning all 41 slide/layout/master XML parts under `render-improvement-harness/decks` that contain
`custGeom`:

- 959 `a:path` elements, **all** with both `w` and `h`, and **every** `pathLst` holds exactly one
  path. No multi-subpath shapes, and no `fill=`/`stroke=` path attributes to honour.
- 67,809 `a:pt` coordinates, **zero** of them non-numeric — no point in this corpus resolves through
  a guide name. The `T0..Tn` and `connsiteX/Y` guides that several findings cite are real, but they
  feed `a:cxnLst` (connection sites), not the drawn outline; the paths beside them use literal
  integers. Two findings state those guides "reach the drawn outline" — **that part is not
  confirmed, and the XML contradicts it.**
- Command mix: 58,552 `lnTo`, 2,673 `cubicBezTo`, 1,238 `moveTo`, 828 `close`. **Zero `arcTo` and
  zero `quadBezTo`.**

So a port that handles `moveTo`/`lnTo`/`cubicBezTo`/`close` with literal coordinates fixes all 17
findings; guides, `arcTo` and `quadBezTo` are correctness work for decks outside this corpus.

### Two findings in this cluster will not be fully fixed by this change

- `cisco-cloud-security/13/2` ("icon entirely missing"): both `custGeom` shapes on that slide use
  `<a:grpFill/>` (`Freeform 39` id 104, `Freeform 40` id 105). With no resolved fill and
  `<a:ln w="0"><a:noFill/>`, the raster paints nothing at
  `crates/pptx-raster/src/lib.rs:376-386` whatever the path is. Blocked behind
  `fill-grpfill-not-resolved`. `cisco-cloud-security/11` is the same story at scale: 42 `custGeom`
  shapes against 50 `grpFill` uses, which is why that finding reads "or not drawn at all".
- `project17/04/3`: `Freeform 57`, the purple lens, is `<a:solidFill><a:srgbClr val="48365A"/>` but
  is invisible in the candidate even as a rectangle. Document order inside `Group 46` is 57, 58, 59,
  16, so the two circles paint over it — yet LibreOffice shows the lens on top. **Not confirmed**;
  this looks like a separate z-order or alpha-compositing question and was not investigated here.

## Verification

Re-render with `.venv/bin/python render-improvement-harness/scripts/render_bo.py <deck>` then
`diff.py <deck>` for `cisco-cloud-security` (02, 03, 04, 05, 06, 07, 11, 13, 16, 19, 20),
`ocp-psp-plan` (01, 03), `project17` (04, 08, 11) and `project20` (16).
`cisco-cloud-security/04` and `/19` carry the most pixels — their `diff_pct` (9.66 and 8.7) should
drop substantially, and the three top hot cells on slide 04 (r1c1 17.0%, r1c2 20.4%, r1c3 16.5%)
should collapse. `project17/08` (hexagons) and `ocp-psp-plan/01` (donut) are the cleanest
pass/fail reads because the shapes are large and unobstructed.

Watch the display-list size while doing it. Worst slides by emitted command count:
`cisco-cloud-security/04` and `/19` at 131 shapes / 18,569 commands each, `project17/11` at 388
shapes / 10,378 commands; worst single shape 461 commands (`cisco-cloud-security/07`). That is
roughly a megabyte of extra JSON per slide on the snapshot and composed paths.

Existing coverage to extend:

- `crates/pptx-parse` has no `custGeom` test at all — the only occurrences of the string under
  `crates/` are `crates/docx-parse/src/drawingml.rs:754`, `crates/docx-parse/src/drawingml.rs:765`
  and `crates/pptx-parse/src/drawing.rs:342`. A parse test beside
  `seeds_standard_geometry_guides_from_extent` (`crates/pptx-parse/src/drawing.rs:1009`) is new
  ground.
- `crates/pptx-render/src/layout.rs:2152-2154` already asserts some emitted `Primitive::Shape` has a
  non-empty path; tighten it to assert a `custGeom` shape's path is not the four-command rect.
- `crates/pptx-raster/src/lib.rs:953` (`geometry_commands_scale_by_the_primitive_box`) already pins
  the unit-space to pixel contract the parsed path must satisfy; a golden beside
  `crates/pptx-raster/tests/golden.rs` for a non-convex freeform would catch winding-rule
  regressions.
- `crates/pptx-render/src/chart.rs:485` and `crates/pptx-render/src/chart.rs:594` assert on
  `geometry == "custom"` primitives from the chart path; make sure a shape-side change does not
  disturb those.
