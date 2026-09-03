---
id: chart-category-order-reversed
title: Plot a horizontal bar chart's first category at the bottom, not the top
category: chart
impact: medium
effort: easy
confidence: high
status: open
occurrences: 3
decks: [stacked-bar]
findings: [stacked-bar/02/3, stacked-bar/03/3, stacked-bar/04/1]
files: [crates/ooxml-drawingml/src/chart/geometry.rs]
---

## Symptom

On a horizontal bar chart (`c:barDir val="bar"`) whose category axis declares the default
`c:orientation val="minMax"`, BetterOffice draws category index 0 at the top of the plot and the
last category at the bottom. PowerPoint and the LibreOffice reference draw the same axis the other
way up: index 0 sits at the axis minimum, which on a left-hand vertical axis is the bottom edge.
The whole plot reads upside down (evidence-1.png, evidence-2.png).

The reference's rows top-to-bottom are `Category 4`, `Category 3`, `Category 2`, `Category 1`; the
candidate's are `Category 1`, `Category 2`, `Category 3`, `Category 4`. The candidate's category
labels are truncated to `Categ` by the separate `chart-axis-position-swapped` defect, so the order
is read off the bar lengths and off the Series 3 end-labels (`2`, `2`, `3`, `5` top-to-bottom in
the candidate, which is Series 3's values in category order 1 to 4).

Measured on the 1280x720 renders of every slide in the deck, plot origin `x=193` (reference) and
`x=119` (candidate), longest contiguous bar run per row, top to bottom:

| row | reference px | reference category | candidate px | candidate category |
|---|---|---|---|---|
| 1 | 856 | Category 4 (12.3) | 685 | Category 1 (8.7) |
| 2 | 577 | Category 3 (8.3)  | 701 | Category 2 (8.9) |
| 3 | 619 | Category 2 (8.9)  | 653 | Category 3 (8.3) |
| 4 | 605 | Category 1 (8.7)  | 968 | Category 4 (12.3) |

Both sets are consistent to a pixel with the stacked totals (reference 69.6 px/unit over its
0-14 axis, candidate 78.7 px/unit over its 0-12.3 axis), so the ordering is established, not
inferred from appearance. The two px/unit figures differ only because of
`chart-axis-autoscale-not-rounded`.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | stacked-bar/02 | reference vs candidate over the whole chart frame: the long `Category 4` bar is the top row on the left and the bottom row on the right |
| 2 | stacked-bar/02 | the two plot areas at 1.4x, reference stacked above candidate: the reference's long/short/mid/mid row pattern against the candidate's exact mirror, with the candidate's Series 3 end-labels reading `2 2 3 5` downward |
| 3 | stacked-bar/04 | the same failure on the deck's other slides (04 shown; 01, 02 and 03 are the same chart recoloured) |

## Root cause (confirmed)

`category_position` treats "no `maxMin`" as "draw index 0 first" in whatever direction the caller
happens to lay its slots out. That is right for every chart type whose categories run along x, and
wrong for the one type whose categories run down the screen.

```rust
/// Drawing position of the `index`th category, which a reversed category axis
/// counts from the far end.
fn category_position(family: PlotFamily<'_>, index: usize, count: usize) -> usize {   // geometry.rs:1946
    if family.category_axis.is_some_and(|axis| axis.reversed) {                       // geometry.rs:1947
        count.saturating_sub(1).saturating_sub(index)
    } else {
        index
    }
}
```

`reversed` is `c:orientation val="maxMin"` and nothing else
(`crates/ooxml-drawingml/src/chart/parse.rs:552`, field at
`crates/ooxml-drawingml/src/chart/model.rs:238`, copied onto `PlotAxis` at
`crates/ooxml-drawingml/src/chart/geometry.rs:539`). This deck's `c:catAx` declares
`minMax`, so `reversed` is `false` and the function returns `index` unchanged.

`emit_bar` then turns that index into a downward y offset:

```rust
let slot = bands.slot * category_position(family, cat_idx, cat_count) as f64;   // geometry.rs:2011
// horizontal branch:
push_text(ops, &label, plot.x - 38.0, plot.y + slot + bands.slot * 0.55, 36.0, category_style);  // 2014-2021
let y = plot.y + offset;                                                        // geometry.rs:2045
push_rect(ops, x0.min(x1), y, (x1 - x0).abs(), bands.bar, &color);              // geometry.rs:2046
```

`plot.y` is the top edge, so slot 0 lands at the top. On a left-hand category axis the axis
minimum is the *bottom* edge, so slot 0 belongs at `plot.y + plot.h`. Screen y and the axis
run in opposite directions, and nothing in the code accounts for that.

Every other `category_position` caller maps the slot onto x, which runs the same way as the axis,
so all of them are already correct and must not change:
`line_x` (`crates/ooxml-drawingml/src/chart/geometry.rs:2094`), `emit_radar`
(`crates/ooxml-drawingml/src/chart/geometry.rs:2489`), `emit_stock`
(`crates/ooxml-drawingml/src/chart/geometry.rs:2624`) and `emit_surface`
(`crates/ooxml-drawingml/src/chart/geometry.rs:2758`, `:2783`). The one transposed caller is
`emit_bar`'s `horizontal` branch.

The property is reachable and the guard is exact. `emit_family` dispatches `"bar"` to
`emit_bar(..., true)` and everything unrecognized - including `"column"` - to
`emit_bar(..., false)` (`crates/ooxml-drawingml/src/chart/geometry.rs:1312-1313`), and
`plot_type_for` produces `"bar"` only for `c:barChart` with `c:barDir val="bar"`
(`crates/ooxml-drawingml/src/chart/parse.rs:580-590`). So `family.chart_type == "bar"` is
equivalent to `emit_bar`'s `horizontal` flag, and is available inside `category_position` without
threading a new argument. `group_category_axis`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1107`) resolves this deck's `c:catAx` from the
`c:barChart`'s two `c:axId`s, so `family.category_axis` is populated and its `reversed` is
readable on the same path.

All four slides carry byte-identical chart geometry - `c:barDir val="bar"`,
`c:grouping val="stacked"`, `catAx` `579760832` with `minMax` / `axPos="l"`, `valAx` `579764440`
with `minMax` / `axPos="b"`, and the same three series
(`4.3 2.5 3.5 4.5` / `2.4 4.4 1.8 2.8` / `2 2 3 5`) - and the measurement above reproduces to the
pixel on all four. Slide 01 therefore shows the same defect even though its slide report did not
raise a finding for it; that is a gap in `stacked-bar/01`, not a difference in the renderer.

Not confirmed / out of scope:

- **Series lane order within a clustered horizontal bar group.** `emit_bar` stacks the non-stacked
  series lanes downward inside a slot (`offset = slot + bands.lead + bands.step * lane`,
  `crates/ooxml-drawingml/src/chart/geometry.rs:2041`). PowerPoint draws the first series of a
  clustered *bar* group at the bottom of the group, so that direction is probably flipped too.
  This deck is `grouping="stacked"`, so every `lane` is `0` and the evidence cannot show it. Called
  out because a reviewer will ask, not because it is proven.
- **Stacked segment order along the value axis.** Series 1 is the leftmost segment in both renders
  (evidence-2.png), so `stacked_spans` is already right and needs no change.
- **Whether the flip belongs on the family or on `axPos`.** The solution keys it on the family
  (`chart_type == "bar"`), matching the sibling `chart-axis-position-swapped` issue, which proposes
  the same `PlotFamily::transposed()` predicate. Keying it on `catAx`'s `axPos == "left"` would give
  the same answer on this deck and on every well-formed file, but would desynchronise from the bars
  on a malformed one.
- **Interaction with the sibling clusters.** This is independent of
  `chart-axis-position-swapped`, `chart-dlbls-shown-when-disabled`,
  `chart-legend-and-title-position-wrong` and `chart-axis-autoscale-not-rounded`. Fixing this one
  alone leaves the value ticks in the left gutter and the labels clipped to `Categ`; fixing that
  one alone leaves the order upside down. Both are needed before the deck reads like the reference.

## Verification

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py stacked-bar
.venv/bin/python render-improvement-harness/scripts/diff.py stacked-bar
```

All four slides sit at `fine_pct` 14.46 today with hot cells `r2c2` 37.0, `r3c2` 36.7, `r3c3` 30.2,
`r2c3` 27.4 (`render-improvement-harness/decks/stacked-bar/diff-summary.json`). Reordering the rows
moves large blocks of colour, so this fix alone should visibly cut `fine_pct` in the plot cells -
but the axis and autoscale defects keep the bar *lengths* different from the reference, so do not
expect the number to approach zero. The check that settles it is positional:

- the longest bar (`Category 4`, total 12.3) must become the **bottom** row and the `Category 1`
  bar the top one, matching evidence-1.png's left panel;
- with `chart-axis-position-swapped` also fixed, the left gutter must read `Category 4`,
  `Category 3`, `Category 2`, `Category 1` downward.

`stacked-bar` is the only `barDir="bar"` deck in the harness, so no other deck should move at all.
A diff change elsewhere means a non-bar path was disturbed.

Existing tests that cover this area and must keep passing unchanged - all of them use `"column"`,
`"line"` or `"pie"` families, so none of them constrains the transposed direction:

- `a_reversed_category_axis_draws_the_categories_from_the_far_end`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4831`) - the only test that exercises
  `category_position`'s reversal, and it builds a **column** chart, so it is unaffected by a
  bar-only flip;
- `stacked_bars_pile_onto_one_another`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4389`),
  `percent_stacked_normalizes_every_category`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4409`) and
  `gap_width_and_overlap_size_and_place_the_bars`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4426`) - all `"column"`;
- `column_chart_emits_background_title_axes_bars_and_legend`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3402`);
- the `"bar"` entries in `axis_titles_draw_beside_the_axes_they_name`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3454`) and in `every_family_stays_finite_and_bounded_on_hostile_input`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:5357`) only count or bound-check ops, they do not
  assert positions;
- consumer smoke tests `every_parsed_chart_type_plots_into_primitives`
  (`crates/pptx-render/src/chart.rs:439`) and the `["column", "bar"]` loop at
  `crates/pptx-render/src/chart.rs:491`, which only assert that `rect` and `line` geometries appear.

No golden fixture renders a horizontal bar chart: the only `barDir="bar"` string in the crates is a
parse-level test at `crates/pptx-parse/src/chart.rs:97`, and
`crates/pptx-raster/tests/golden/chart.png` is driven by a non-bar chart. So no golden needs
rebaselining, and the new assertion - that a `"bar"` family's first category draws below its last -
is the one this issue adds.
