---
id: geometry-preset-adj-values-wrong
title: Scale preset adjust values off the shortest side, not the width
category: geometry-preset
impact: low
effort: easy
confidence: high
status: open
occurrences: 1
decks: [project20]
findings: [project20/04/2]
files: [crates/ooxml-drawingml/src/geometry.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/lib.rs, crates/pptx-edit/src/deck.rs, crates/docx-parse/src/drawingml.rs]
---

## Symptom

On `project20/04` the "Week Ending" ribbon is a row of seven `chevron`/`homePlate` banners whose
bounding boxes deliberately overlap, so each shape's point tucks into the next shape's notch and the
row reads as one continuous blue band. BetterOffice draws every notch roughly twice as deep as it
should be, so each of the six joints opens into a large white bowtie of slide background
(`evidence-1.png`, and at 4x in `evidence-2.png`). The same defect hits the dark-navy phase row
below, where the `homePlate` arrows are wide and short: their points are drawn three to five times
too long, turning stubby banners into long spikes (`evidence-3.png`, `evidence-4.png`).

The error grows with the shape's aspect ratio, which is why the failure is invisible on square-ish
presets and severe on these banner shapes.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project20/04 | The whole week-ending chevron ribbon. Reference: hairline seams. Candidate: a wide white gap at every one of the six joints. |
| 2 | project20/04 | 4x on the `April 27`/`May 4` joint (`Arrow: Pentagon 60` id 61 into `Arrow: Chevron 47` id 48) - the clearest single view of the over-deep chevron notch. |
| 3 | project20/04 | The dark-navy phase row (`homePlate`, default `adj`); candidate points are visibly several times longer than the reference's. |
| 4 | project20/04 | 4x on `Arrow: Pentagon 46` (id 47, `Content Review`): reference point 19 px, candidate point 67 px. |

Measured on the 1280x720 renders (`decks/project20/{lo,bo}-img/04.png`), scanning where each
shape's flat top edge starts to slope:

| shape | w x h (px) | reference | candidate | ECMA formula | code's formula |
|---|---|---|---|---|---|
| `Arrow: Chevron 47` (id 48, `chevron`, default `adj`) | 171.3 x 55.6 | notch 27.5 px | notch 57.5 px | `ss * 0.5` = 27.8 | `w * 0.35` = 60.0 |
| `Arrow: Pentagon 46` (id 47, `homePlate`, default `adj`) | 280.9 x 37.5 | point 19.1 px | point 67.1 px | `ss * 0.5` = 18.8 | `w * 0.25` = 70.2 |
| `Arrow: Pentagon 57` (id 58, `homePlate`, default `adj`) | 393.3 x 37.4 | point 18.3 px | point 93.3 px | `ss * 0.5` = 18.7 | `w * 0.25` = 98.3 |

The candidate column matches the code's formula to within antialiasing, and the reference column
matches the ECMA formula, in every row. Consecutive chevrons overlap by 24.6 px, so a 27.5 px notch
leaves the reference's 3 px seam while a 57.5 px notch leaves the 34 px gap measured in the
candidate.

## Root cause (hypothesis)

**Confirmed. `preset_geometry_to_path` treats an `adj` guide as a fraction of the shape's *width*;
ECMA-376 defines it as a fraction of the shape's *shortest side* (`ss`).**

1. Parsing is correct and is not the problem. `parse_adjust_values`
   (`crates/pptx-parse/src/drawing.rs:355`) evaluates the `prstGeom/avLst` guides and divides a
   plain `val` by `ADJUSTMENT_SCALE`, so `<a:gd name="adj" fmla="val 47414"/>` reaches the model as
   `0.47414` (`crates/pptx-parse/src/drawing.rs:377-386`). `standard_guide_values`
   (`crates/pptx-parse/src/drawing.rs:393`) even seeds a correct `ss = min(w, h)`
   (`crates/pptx-parse/src/drawing.rs:394`, `crates/pptx-parse/src/drawing.rs:398`) for guides that
   reference it. Nothing is dropped.

2. The value is then consumed as a width fraction. The chevron arm
   (`crates/ooxml-drawingml/src/geometry.rs:199-209`) builds
   `polygon(&[(0,0), (1-adj,0), (1,0.5), (1-adj,1), (0,1), (adj,0.5)])`. The polygon *topology* is
   exactly the ECMA `chevron` path (`moveTo 0,0 -> x2,0 -> r,vc -> x2,b -> 0,b -> x1,vc`), but the
   x coordinates emitted here are fractions of the shape's width - `crates/pptx-raster/src/lib.rs:575`
   and its `px()` helper multiply x by `w`. The spec's `x1` is `*/ ss a 100000`, i.e. `adj` times the
   *shortest side*. For `Arrow: Chevron 47` (`w/h = 3.08`) that alone is a 3.08x overshoot, before
   the default-value error below.

3. `homePlate` ignores `adj` entirely. `crates/ooxml-drawingml/src/geometry.rs:210` is a constant
   `polygon(&[(0,0), (0.75,0), (1,0.5), (0.75,1), (0,1)])`, and
   `preset_geometry_default_adjustments` (`crates/ooxml-drawingml/src/geometry.rs:8-30`) has no
   `homePlate` arm at all, so the explicit `<a:gd name="adj" fmla="val 47414"/>` on
   `Arrow: Pentagon 60` (id 61) is parsed, carried through the model, and then discarded at the
   geometry table. Every `homePlate` on the slide gets the same hardcoded `w * 0.25` point.

4. The `chevron` default is also wrong. `crates/ooxml-drawingml/src/geometry.rs:19` returns
   `("adj", 0.35)`; the ECMA-376 `chevron` preset defines `<gd name="adj" fmla="val 50000"/>`, i.e.
   `0.5`. This is a second, independent error stacked on top of (2) - on `Arrow: Chevron 47` the two
   partly cancel (`0.35 * w` = 60.0 px against the correct `0.5 * ss` = 27.8 px, a 2.16x overshoot
   rather than 3.08x), which is why the gaps are large but not catastrophic.

5. `.min(0.5)` at `crates/ooxml-drawingml/src/geometry.rs:200` is the wrong clamp for the same
   reason: ECMA pins `a` to `[0, maxAdj]` where `maxAdj` is itself `ss`-relative, so a legitimate
   `adj` above `0.5` on a wide shape is silently truncated today. **Not confirmed**: no deck in the
   corpus carries a `chevron` or `homePlate` with `adj > 50000`, and I have no local copy of
   `presetShapeDefinitions.xml`, so the exact `maxAdj` numerator (I believe `*/ 100000 w ss`, giving
   `dx <= w`, for both presets) must be read off the spec before the clamp is rewritten.

6. One fix covers every surface. Both `pptx-render` call sites already pass the aspect ratio
   (`crates/pptx-render/src/layout.rs:410-414`, `crates/pptx-render/src/layout.rs:516-520`, and the
   composed path at `crates/pptx-render/src/lib.rs:170`, all funnelling into the two `geometry_path`
   helpers at `crates/pptx-render/src/layout.rs:1946` and `crates/pptx-render/src/lib.rs:237`), so
   `preset_geometry_to_path` already has the `w/h` it needs and nothing above it has to change.
   There is no second preset table under `packages/` - the web canvas consumes the same command
   list - so raster and canvas are fixed together.

Secondary, outside this cluster's evidence but caused by the same table: `pptx-edit` seeds a new
preset shape's adjust values from `preset_geometry_default_adjustments`
(`crates/pptx-edit/src/deck.rs:412`) and merges them into every parsed shape's document state
(`crates/pptx-edit/src/deck.rs:126-130`), and `set_adjust_values`
(`crates/pptx-parse/src/write.rs:1225`) writes them back as `val <value * 100000>`. A chevron
authored in the editor therefore lands in the file as `val 35000` instead of `val 50000`. Existing
shapes are safe - `crates/pptx-edit/src/save.rs:214` diffs two snapshots that both carry the
injected default, so no spurious patch is produced - but the wrong default does escape into newly
authored XML.

Same class, sharing the width-vs-`ss` basis error and worth checking in the same pass (**not
confirmed - no finding in the corpus backs them**): `parallelogram`
(`crates/ooxml-drawingml/src/geometry.rs:104`), `trapezoid`
(`crates/ooxml-drawingml/src/geometry.rs:108`), `hexagon`
(`crates/ooxml-drawingml/src/geometry.rs:113`) and `octagon`
(`crates/ooxml-drawingml/src/geometry.rs:125`) all use `adj` as a plain width (and, for `octagon`,
height) fraction, and all four are `ss`-relative in ECMA-376.

## Verification

Re-render `project20/04` (`.venv/bin/python render-improvement-harness/scripts/pipeline.py`, or
`render_bo.py` for that deck alone) and compare against `decks/project20/lo-img/04.png`. The
week-ending ribbon must close: the white run at each of the six seams, sampled along the row's
vertical centre (`y = 214` at 1280x720), must fall from ~34 px to the reference's ~3 px, and the
navy phase row's `homePlate` points must shorten from ~67 px to ~19 px on `Arrow: Pentagon 46`. The
slide's `diff_pct` is 8.89 today; this row is one of six defects on the slide, so expect a partial
drop, not a clean pass.

Unit coverage in `crates/ooxml-drawingml/src/geometry.rs` currently stops at `roundRect`,
`parallelogram` defaults and `flowChartTerminator`
(`crates/ooxml-drawingml/src/geometry.rs:497-561`); there is no `chevron` or `homePlate` test at
all. Add aspect-ratio assertions in that module (a 3.08:1 chevron with the default `adj` must put
its notch at `0.5 / 3.08 = 0.1624` of the width, not `0.35`) - that is the cheapest regression
guard, and it pins the `ss` basis rather than the pixel output.
