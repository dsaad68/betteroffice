---
id: chart-axis-position-swapped
title: Draw a horizontal bar chart's value axis along the bottom, not down the left
category: chart
impact: medium
effort: medium
confidence: high
status: open
occurrences: 4
decks: [stacked-bar]
findings: [stacked-bar/01/2, stacked-bar/02/2, stacked-bar/03/2, stacked-bar/04/2]
files: [crates/ooxml-drawingml/src/chart/geometry.rs, crates/pptx-render/src/chart.rs, crates/xlsx-render/src/chart.rs, crates/docx-layout/src/display_list.rs]
---

## Symptom

On a horizontal bar chart (`c:barDir val="bar"`), the shared plot geometry lays the value axis out
vertically, exactly as it does for a column chart. The tick labels (`0`, `3.1`, `6.2`, `9.2`,
`12.3`) land in the same left-hand column as the category names, interleaved with them one for one,
and the bottom edge of the plot gets no labels at all (evidence-1.png, evidence-3.png).

Both label sets are pushed at the identical x — `plot.x - 38.0` — so they occupy the same 38px
gutter. The category names overflow that gutter into the plot area and the bars, drawn immediately
after each label, paint over the overflow: every `Category N` reads `Categ` (evidence-2.png). In
the reference the same names get ~70px of gutter (x in [99,168] of a 1280px render) and the value
ticks sit on their own row at y in [560,572], below the plot.

All four slides of `stacked-bar` are the same chart in four colourways, so the defect is identical
on each (evidence-4.png shows slide 04; 02 and 03 differ only in the series palette).

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | stacked-bar/01 | reference vs candidate over the whole axis region: `Category 1`-`Category 4` spelled out on the left with ticks `0`-`14` under the plot, against the candidate's single column of alternating `Categ` / value ticks and a bare bottom edge |
| 2 | stacked-bar/01 | 2x zoom on the left gutter: the value ticks sit between the category rows, and every category name is cut at the plot edge where the bar paints over it |
| 3 | stacked-bar/01 | 1.4x zoom of the bottom band: the reference's `0 2 4 6 8 10 12 14` row against the candidate's empty one |
| 4 | stacked-bar/04 | the same failure on the deck's other slides (04 shown; 02 and 03 are the same chart recoloured) |

## Root cause (confirmed)

`c:axPos` is parsed, carried all the way into the plot model, and then never read.

- `parse_axis` maps `c:axPos` to `ChartAxis::position` as `"left" | "right" | "top" | "bottom"`
  (`crates/ooxml-drawingml/src/chart/parse.rs:536`), and `axis_list` collects every `catAx` /
  `dateAx` / `valAx` / `serAx` under the plot area
  (`crates/ooxml-drawingml/src/chart/parse.rs:117`).
- `plot_axis_from_model` copies it onto `PlotAxis::position`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:548`, field at
  `crates/ooxml-drawingml/src/chart/geometry.rs:288`).
- Nothing reads it. Grepping `.position` across `geometry.rs` returns only that assignment, plus
  data-label positions (`PlotDataLabels::position`,
  `crates/ooxml-drawingml/src/chart/geometry.rs:362`), the legend position
  (`crates/ooxml-drawingml/src/chart/geometry.rs:242`, read at
  `crates/ooxml-drawingml/src/chart/geometry.rs:799`) and `Iterator::position` calls. The axis
  field is dead data.

The value axis is therefore always drawn as if it were vertical. `emit_axes`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1649`) hard-codes one orientation:

```rust
let label_x = if family.secondary {
    plot.x + plot.w + 4.0
} else {
    plot.x - 38.0            // geometry.rs:1669
};
// ...
let y = scale.y(plot, value);                                            // 1685
push_line(ops, plot.x, y, plot.x + plot.w, y, CHART_GRID_COLOR, 0.5);    // 1687 - gridlines run across
push_text(ops, &scale.format(value, number_format), label_x, y + 3.0, 34.0, tick_style);  // 1692
```

Tick marks are placed against the vertical edge `plot.x`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1661-1665`,
`crates/ooxml-drawingml/src/chart/geometry.rs:1700`), the value-axis title goes above-left at
`plot.x - 38.0, plot.y - 5.0` (`crates/ooxml-drawingml/src/chart/geometry.rs:1742`) and the
category-axis title below the plot (`crates/ooxml-drawingml/src/chart/geometry.rs:1757`) - all four
placements assume value-vertical / category-horizontal.

`emit_bar` already knows the family is transposed: `emit_family` dispatches `"bar"` with
`horizontal = true` and everything else with `false`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1312-1313`, fed by `plot_type_for`'s `barDir`
check at `crates/ooxml-drawingml/src/chart/parse.rs:584`). It uses that flag for the bars
(`scale.x` instead of `scale.y`, `crates/ooxml-drawingml/src/chart/geometry.rs:2043-2046`) and for
the category labels, which it moves to the left gutter
(`crates/ooxml-drawingml/src/chart/geometry.rs:2013-2021`). But it calls
`emit_axes(ops, family, plot)` unconditionally and without the flag
(`crates/ooxml-drawingml/src/chart/geometry.rs:2002`), so the value axis stays vertical while the
bars and categories transpose. That single omission is the whole defect.

The collision is exact, not approximate: the value tick labels at
`crates/ooxml-drawingml/src/chart/geometry.rs:1669` and the horizontal-bar category labels at
`crates/ooxml-drawingml/src/chart/geometry.rs:2017` both evaluate to `plot.x - 38.0`. Measured on
`bo-img/01.png`, every text run in the left gutter starts at x=80, i.e. `plot.x = 118`.

The `Categ` truncation is a paint-order effect, not a clip. `push_text` truncates only at 120
chars (`crates/ooxml-drawingml/src/chart/geometry.rs:1349`) and `chart_text_primitive` widens the
box to the measured run rather than clipping it
(`crates/pptx-render/src/layout.rs:1134`: `w: safe_geometry(text.width as f32).max(width)`). The
label is emitted first and the bar rect for the same category immediately after
(`crates/ooxml-drawingml/src/chart/geometry.rs:2012` then
`crates/ooxml-drawingml/src/chart/geometry.rs:2046`), so the opaque bar covers everything past
`plot.x`. Widening the gutter is therefore part of the fix, not a separate cosmetic issue: the plot
rect reserves a fixed 42px on the left (`crates/ooxml-drawingml/src/chart/geometry.rs:805-818`)
regardless of chart type, and `Category 4` needs ~70px.

The property is reachable on this deck's code path, so a fix keyed on `axPos` will actually see a
value. The `c:barChart` carries both `c:axId`s, `parse_plot_group` collects them
(`crates/ooxml-drawingml/src/chart/parse.rs:616`), and `group_value_axes` /
`group_category_axis` resolve them to the two definitions
(`crates/ooxml-drawingml/src/chart/geometry.rs:1071`,
`crates/ooxml-drawingml/src/chart/geometry.rs:1107`). For `stacked-bar` that yields
`family.axis.position == Some("bottom")` and `family.category_axis.position == Some("left")`,
from:

```xml
<c:barChart><c:barDir val="bar"/><c:grouping val="stacked"/>...
<c:catAx><c:axId val="579760832"/>...<c:axPos val="l"/>...<c:crossAx val="579764440"/></c:catAx>
<c:valAx><c:axId val="579764440"/>...<c:axPos val="b"/>...<c:crossAx val="579760832"/></c:valAx>
```

identical in all four of `xml/01/chart-chart1.xml` ... `xml/04/chart-chart4.xml`.

Not confirmed / judgement calls:

- **Whether the fix should key on `axPos` or on `barDir`.** Both give the same answer on every
  well-formed file, and this deck cannot distinguish them. `axPos` is the physical edge and is the
  spec answer, but keying the axes off it alone lets a malformed deck (`barDir="bar"` with
  `axPos="l"` on the value axis) transpose the axes while `emit_bar` keeps drawing horizontal
  bars - a worse render than today's. The solution below keys the orientation off the family
  (which already decides the bars) and uses `axPos` to pick the side within that orientation; that
  is a choice, not something the evidence settles.
- **Secondary value axes on a transposed family.** `emit_axes` puts a secondary axis on the right
  edge (`crates/ooxml-drawingml/src/chart/geometry.rs:1661-1667`,
  `crates/ooxml-drawingml/src/chart/geometry.rs:1712-1723`); transposed it belongs on the top. No
  deck here has a secondary axis on a bar chart, so this is reasoned from the code only.
- **Gridline direction.** The value gridlines would also have to become vertical. Invisible in
  this evidence: `c:valAx` here declares no `c:majorGridlines`, so `major_grid` is false
  (`crates/ooxml-drawingml/src/chart/geometry.rs:1657`) and none are drawn in either render.
- **Category order** (`stacked-bar/02/3`, `stacked-bar/03/3`, `stacked-bar/04/1`) is the separate
  `chart-category-order-reversed` cluster, and the legend/title placement visible in evidence-4 is
  `chart-legend-and-title-position-wrong`. The tick values themselves (`3.1`, `6.2`, ... instead of
  `2`, `4`, ...) are `chart-axis-autoscale-not-rounded`. None of the three is fixed by this issue,
  and all three will still show in a re-render.

## Verification

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py stacked-bar
.venv/bin/python render-improvement-harness/scripts/diff.py stacked-bar
```

All four slides currently sit at `fine_pct` 14.46 with hot cells `r2c2` 37.0, `r3c2` 36.7, `r3c3`
30.2, `r2c3` 27.4 (`render-improvement-harness/decks/stacked-bar/diff-summary.json`). The left
column is a narrow band of the frame, so the headline number will only partly move; the checks that
matter are positional, not statistical:

- no `PlotOp::Text` from `emit_axes` may share an x with the category labels - after the fix the
  tick labels sit under the plot and the category names get the gutter to themselves;
- the bottom band must carry `0 ... max` (compare against evidence-3.png);
- `Category 1`-`Category 4` must render in full, which only happens if the left gutter grows past
  ~70px.

`stacked-bar` is the only horizontal-bar deck in the harness, so no other deck should move at all;
a diff change elsewhere means the column path was disturbed.

Existing tests that cover this area and must keep passing:

- `column_chart_emits_background_title_axes_bars_and_legend`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3402`) asserts exactly 7 `PlotOp::Line`s for a
  column chart - the column path must be untouched;
- `axis_titles_draw_beside_the_axes_they_name`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3454`) already loops over `"bar"` but only checks
  that the titles exist, not where they land;
- `stacked_bars_pile_onto_one_another` (`crates/ooxml-drawingml/src/chart/geometry.rs:4389`),
  `gap_width_and_overlap_size_and_place_the_bars`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4426`),
  `a_reversed_category_axis_draws_the_categories_from_the_far_end`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4831`),
  `tick_marks_draw_only_when_the_axis_names_them`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4803`),
  `gridlines_follow_the_axis_that_declares_them`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4757`);
- consumer-side smoke tests `every_parsed_chart_type_plots_into_primitives` and
  `wedge_families_draw_closed_paths_and_flat_families_draw_rectangles`
  (`crates/pptx-render/src/chart.rs:438`, `crates/pptx-render/src/chart.rs:470`), plus the
  `chart` raster golden (`crates/pptx-raster/tests/golden/chart.png`, driven by
  `crates/pptx-raster/tests/golden.rs:346`).

No test asserts where a bar chart's value ticks are drawn; that assertion is the new one.
