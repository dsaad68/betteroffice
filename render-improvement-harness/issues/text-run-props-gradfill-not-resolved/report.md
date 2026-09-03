---
id: text-run-props-gradfill-not-resolved
title: Resolve run-level gradFill instead of dropping it
category: text-run-props
impact: high
effort: easy
confidence: high
status: open
occurrences: 9
decks: [project20, rollout-plan]
findings: [project20/01/2, project20/05/3, project20/07/4, project20/09/4, project20/11/4, rollout-plan/02/1, rollout-plan/04/2, rollout-plan/05/2, rollout-plan/08/1]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/pptx-parse/src/write.rs, crates/pptx-edit/src/story.rs, crates/pptx-render/src/layout.rs]
---

## Symptom

Text runs whose `a:rPr` carries an `a:gradFill` instead of an `a:solidFill` render in the theme's
default body-text color rather than the gradient's color. Every occurrence in these two decks is
the same authoring pattern: a degenerate two-stop gradient with both stops `FFFFFF`, i.e. solid
white, used for label text sitting on a saturated fill. Because the fallback color resolves to
`dk1` (`#505050` in both decks' themes), the text turns dark-gray-on-red, dark-gray-on-blue and
dark-gray-on-purple - barely legible (evidence-1.png, evidence-2.png, evidence-3.png,
evidence-4.png).

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project20/01 | the red "Please work on noted slides only." callout: white in the reference, `#505050` gray in the candidate |
| 2 | project20/05 | the "Solutions" heading on its `accent1` blue band, gray instead of white |
| 3 | project20/07 | the full-height red instructional callout, every line gray instead of white |
| 4 | rollout-plan/08 | the three column headers "Business Lead" / "Business Contact" / "Team Member" on purple, green and blue bands, all gray instead of white |

## Root cause (hypothesis)

Run-level fill is parsed as *solid fill only*. `parse_run_properties` reads exactly one fill
element:

- `crates/pptx-parse/src/drawing.rs:917` - `color: element.child("solidFill").and_then(parse_color_container)`

There is no `gradFill` branch anywhere on the run path, so for these runs `RunProperties.color`
(`crates/pptx-parse/src/model.rs:344`) stays `None`. The gradient itself is never read: the
`gradFill` handling that does exist (`crates/pptx-parse/src/drawing.rs:576`, and
`parse_gradient_fill` at `crates/pptx-parse/src/drawing.rs:585`) sits inside `parse_fill`
(`crates/pptx-parse/src/drawing.rs:565`), which is only reached for shape `spPr` and background
fills. Its `ShapeFill` result would have nowhere to live on a run anyway - `RunProperties` carries
a bare `Option<ColorValue>`, not a fill.

Downstream, the `None` propagates unchanged. `style_from_run_properties`
(`crates/pptx-edit/src/story.rs:643`) maps `properties.color` straight onto the snapshot's
`TextStyle.color`, and `resolve_style` (`crates/pptx-render/src/layout.rs:1042`) then falls back to
the inherited `defRPr` color before its final `"#000000"` default. For `project20` slide 1 that
inherited value is the master's `otherStyle` `lvl1pPr/defRPr`
`<a:solidFill><a:schemeClr val="tx1"/></a:solidFill>`, which the deck's `clrMap` sends to `dk1` =
`505050` - exactly the gray sampled in every slide report of this cluster. Confirmed against the
extracted XML: `decks/project20/xml/01/master.xml` carries that `defRPr`, and
`decks/project20/xml/01/theme.xml` has `<a:dk1><a:srgbClr val="505050"/></a:dk1>`.

The property the findings cite is reachable on that exact path - it is a direct child of `a:rPr`
(`decks/project20/xml/01/slide.xml`):

```xml
<a:rPr lang="en-US" sz="2400">
  <a:gradFill><a:gsLst>
    <a:gs pos="0"><a:srgbClr val="FFFFFF"/></a:gs>
    <a:gs pos="100000"><a:srgbClr val="FFFFFF"/></a:gs>
  </a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill>
  <a:ea typeface="Segoe UI"/><a:cs typeface="Segoe UI"/>
</a:rPr>
```

All nine findings share that shape: two stops, both `FFFFFF`, `lin ang="5400000" scaled="0"`.

Two secondary observations, both flagged as such:

- The display list cannot express gradient-filled text at all: `TextRun.color`
  (`crates/pptx-render/src/display_list.rs:204`) and `PositionedTextRun.color`
  (`crates/pptx-render/src/display_list.rs:242`) are plain hex `String`s. A genuinely
  non-degenerate run gradient would still need a flattening approximation after this fix. Every
  occurrence in this cluster is degenerate, so flattening is exact here.
- **Not confirmed / separate gap:** the failing shapes also declare
  `<p:style><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style>`, which would
  independently yield white. `fontRef`, `fillRef` and `lnRef` appear nowhere in `pptx-parse`,
  `pptx-render` or `ooxml-drawingml` (grep returns nothing), so the style-matrix fallback is a
  second, unrelated hole. Fixing `gradFill` is sufficient for these nine findings; `fontRef` is
  not part of this issue.

## Verification

Re-render `project20` slides 01, 05, 07, 09, 11 and `rollout-plan` slides 02, 04, 05, 08, then check
that the affected runs sample as `(255, 255, 255)` instead of `(80, 80, 80)`. `project20/09`
(diff 5.25%) and `project20/11` (4.5%) are the cleanest signals because their other findings are
small; `rollout-plan/08` (11.8%) should also drop visibly. `project20/01` (46.33%) will barely move,
since that slide is dominated by the separate `fill-alpha-modifier-ignored` bug.

Unit coverage lives beside the parser: `parses_text_formatting_and_nested_shape_types`
(`crates/pptx-parse/src/drawing.rs:957`) already asserts on `runs[0].properties`, so extend it - or
add a sibling - with an `rPr` carrying a `gradFill` and assert the resolved `color`. There is no
`crates/pptx-render/tests/` directory; raster coverage would go through
`crates/pptx-raster/tests/golden.rs`.

Round-trip is the one thing that must not regress. `apply_run_properties`
(`crates/pptx-parse/src/write.rs:1520`) strips every fill element and writes a `solidFill` whenever
`properties.color.is_some()`; the comment at `crates/pptx-parse/src/write.rs:1542` states that
clearing the color is what currently lets an unmodeled `gradFill` survive a save. Populating
`color` from a `gradFill` without guarding that branch would silently rewrite authored gradients as
solid fills. A save-and-reload test over a `gradFill` run is required as part of the fix.
