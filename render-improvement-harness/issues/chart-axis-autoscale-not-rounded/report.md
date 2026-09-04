---
id: chart-axis-autoscale-not-rounded
title: Pick a rounded major unit for a value axis instead of cutting the range into four
category: chart
impact: low
effort: medium
confidence: high
status: open
occurrences: 4
decks: [stacked-bar]
findings: [stacked-bar/01/4, stacked-bar/02/5, stacked-bar/03/5, stacked-bar/04/3]
files: [crates/ooxml-drawingml/src/chart/geometry.rs, crates/docx-layout/tests/chart_snapshots.rs]
---

## Symptom

The `stacked-bar` chart declares no `c:max`, no `c:min` and no `c:majorUnit`, so both engines have
to invent a scale. LibreOffice picks `0 2 4 6 8 10 12 14`. BetterOffice takes the raw stacked
maximum, 12.3, as the axis end and cuts the range into four equal pieces, giving
`0 3.1 6.2 9.2 12.3` (evidence-1.png, evidence-2.png).

Two things follow from the same decision. The labels read as noise — `9.2` is not even the true
tick value, which is `9.225`, rounded by the one-decimal display rule. And because the axis ends
exactly at the largest value, the largest stack is drawn hard against the plot edge with no
headroom, where the reference leaves it stopping short of the `14` gridline (evidence-3.png).

All four slides of `stacked-bar` carry the same chart in four colourways and fail identically
(evidence-4.png).

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | stacked-bar/01 | the two charts side by side: `0 2 4 6 8 10 12 14` along the reference's bottom edge against the candidate's `0 3.1 6.2 9.2 12.3` |
| 2 | stacked-bar/01 | 1.7x on the two label sets alone — rounded, step 2, against 12.3 in quarters |
| 3 | stacked-bar/01 | the Category-4 stack in both engines at matched plot width: the reference leaves ~12% headroom past 12.3, the candidate has none because `max == 12.3` |
| 4 | stacked-bar/04 | the same failure on the deck's other slides (04 shown; 02 and 03 differ only in palette) |

## Root cause (confirmed)

Two independent gaps, both in `crates/ooxml-drawingml/src/chart/geometry.rs`, and the defect on
this deck needs both fixed.

**1. There is no auto major unit.** `axis_ticks`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1597`) honours `c:majorUnit` when the file names one
(`crates/ooxml-drawingml/src/chart/geometry.rs:1612`) and otherwise falls through to a literal
quartering of the range:

```rust
    (0..=4)
        .map(|step| match step {
            4 => scale.max,
            step => scale.min + span * step as f64 / 4.0,
        })
        .collect()
```

(`crates/ooxml-drawingml/src/chart/geometry.rs:1631-1636`; the doc comment at
`crates/ooxml-drawingml/src/chart/geometry.rs:1595-1596` states the behaviour outright — "at five
even steps otherwise"). Nothing ever asks whether `span / 4` is a number a reader would want to
see. Spreadsheet engines pick a unit from `{1, 2, 2.5, 5} x 10^k` instead.

**2. The auto bounds are the raw data bounds.** `value_scale`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1559`) takes `min`/`max` straight from
`stacked_totals` / `percent_range` / `plain_range`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1483`, `:1507`, `:1530`), overrides either end that
`c:scaling` pins (`crates/ooxml-drawingml/src/chart/geometry.rs:1566-1575`), and returns. When an
end is *not* pinned it stays on the data value; nothing extends it outward to the next unit.

The arithmetic on this deck matches exactly. The three series sum per category to
`8.7, 8.9, 8.3, 12.3`, so `stacked_totals` returns `(0.0, 12.3)`; `c:scaling` carries only
`<c:orientation val="minMax"/>` in all four of `xml/01/chart-chart1.xml` ...
`xml/04/chart-chart4.xml`, and there is no `c:majorUnit` anywhere in them. The quartering therefore
yields `0, 3.075, 6.15, 9.225, 12.3`, which `format_number`
(`crates/ooxml-drawingml/src/chart/geometry.rs:3169-3175`) renders one decimal at a time as
`0, 3.1, 6.2, 9.2, 12.3` — the exact five labels in evidence-2.png.

The same `ValueScale` (`crates/ooxml-drawingml/src/chart/geometry.rs:1427`) positions the bars, so
the un-extended `max` is what pins the longest stack to the plot edge in evidence-3.png:
`emit_bar` maps values with `scale.x` (`crates/ooxml-drawingml/src/chart/geometry.rs:2003`), and
`ValueScale::fraction` returns exactly 1.0 for `value == max`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1444`). Rounding the bounds and rounding the ticks
cannot be done separately: they are the same number.

**The product's own baseline shows gap 1 in isolation.** `crates/docx-layout/tests/chart_snapshots.rs`
builds every case from a chart with an *explicit* `0..25` value axis
(`crates/docx-layout/tests/chart_snapshots.rs:140`) and no major unit, and the committed snapshot
records the quartering:

```
{"baselineY":71,...,"text":"25",...}
{"baselineY":100.5,...,"text":"18.8",...}
{"baselineY":130,...,"text":"12.5",...}
{"baselineY":159.5,...,"text":"6.2",...}
```

(`crates/docx-layout/tests/chart_snapshots.rs:724`, `:726`, `:728`, `:730`). PowerPoint and
LibreOffice both show `0 5 10 15 20 25` there. So the missing auto unit is not specific to auto
bounds — it is wrong on every value axis that does not name `c:majorUnit`, which is most of them.

Reach: `plot_chart_into` (`crates/ooxml-drawingml/src/chart/geometry.rs:759`) is the single entry
point for all three renderers — `crates/pptx-render/src/chart.rs:47`,
`crates/xlsx-render/src/chart.rs:498`, `crates/docx-layout/src/display_list.rs:7975` — so one
change moves charts in every format.

Not confirmed / judgement calls:

- **How many intervals to aim for.** This is the one thing the evidence does not settle, and it
  decides the answer. The textbook Heckbert "nice numbers" with a fixed 5 ticks gives step 5 on
  this data (`0 5 10 15`), not LibreOffice's step 2. Reproducing `0..14 by 2` needs roughly 7
  intervals, and the reference spends ~979px on that axis — about 122px per label. LibreOffice
  and Excel both size the interval count from the axis extent, which is why a horizontal value axis
  gets more ticks than a vertical one. `axis_ticks` has no access to the plot rectangle today
  (`crates/ooxml-drawingml/src/chart/geometry.rs:1597` takes only the scale and the unit), so
  this is the plumbing the fix actually costs. The solution note proposes an extent-derived target;
  the exact divisor is a tuning choice, not a spec value.
- **Whether to extend a pinned end.** The report assumes not: `c:max`/`c:min` are the author's
  numbers and must survive verbatim, which is what `axis_bounds_override_the_data_range`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3517`) already asserts. Only the *unit* is
  auto-picked when the bounds are pinned. That is how the docx snapshot case above should behave.
- **Log axes.** `axis_ticks` already handles a log axis with powers of the base
  (`crates/ooxml-drawingml/src/chart/geometry.rs:1599-1611`) and must keep that path; nice numbers
  do not apply. No deck in the harness exercises it — `a_log_axis_places_values_by_their_logarithm`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4704`) is the only coverage.
- **The scatter x axis has the identical defect**, reasoned from the code only, with no deck to
  show it: `scatter_x_scale` (`crates/ooxml-drawingml/src/chart/geometry.rs:2256`) derives raw
  bounds the same way and `emit_scatter_x_labels`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:2303`) ticks them through the same `axis_ticks`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:2316`). Fixing one should fix both.
- **This cluster is independent of the other three on this deck.** `chart-axis-position-swapped`
  moves these labels to the bottom edge, `chart-dlbls-shown-when-disabled` removes the digits past
  each bar, `chart-legend-and-title-position-wrong` moves the legend. None changes the tick
  *values*. Note the interaction, though: once the value axis runs horizontally, five labels
  `0 / 3.1 / 6.2 / 9.2 / 12.3` sit in one row and can collide, where stacked vertically they did
  not — so the position fix makes this one more visible, not less.

## Verification

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py stacked-bar
.venv/bin/python render-improvement-harness/scripts/diff.py stacked-bar
```

All four slides sit at `fine_pct` 14.46 today
(`render-improvement-harness/decks/stacked-bar/diff-summary.json`). The tick labels are a few
hundred pixels of a 1280x744 frame, so the headline number barely moves; check the values instead:

- the tick set must become `0 2 4 6 8 10 12 14` — eight labels, not five, and no decimal point in
  any of them;
- the longest stack must no longer touch the plot edge; at `max = 14` it should end at
  `12.3 / 14 = 87.9%` of the plot width (evidence-3.png is the reference measurement);
- `stacked-bar` is the only deck in the harness whose value axis is auto on both ends, but every
  deck with a chart re-renders, so any *other* deck whose diff moves is worth reading.

Tests that break by design and must be re-baselined with the new values:

- `an_axis_number_format_reaches_the_tick_labels`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4874`) asserts `"75%"` on a `0..1` axis. `0.75` is
  the third quarter; no nice unit produces it. This one is a direct casualty and needs a different
  assertion.
- `crates/docx-layout/tests/chart_snapshots.rs` — 70 snapshotted configurations, 288 tick-label
  lines, plus every gridline `y` and every bar height that moves with them. The test has no bless
  flag; it panics with the full actual output
  (`crates/docx-layout/tests/chart_snapshots.rs:682-698`), which is what gets pasted back into
  `EXPECTED`. This is the bulk of the work and the reason the effort is not `easy`.

Tests that cover the area and should keep passing unchanged — each is a check that the fix stayed
inside its lane:

- `axis_bounds_override_the_data_range`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3517`) — explicit `c:min`/`c:max` must not be
  rounded outward;
- `column_chart_emits_background_title_axes_bars_and_legend`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3402`) counts `PlotOp::Line`s exactly (7) on a
  `0..20` auto axis, so it will hold only if the new unit is 5 there — a useful tripwire on the
  interval-count choice;
- `stacked_bars_pile_onto_one_another`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4389`) asserts the axis reaches `"25"` on totals
  of 25, and `percent_stacked_normalizes_every_category`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4409`) asserts `"100%"` — both survive any unit
  that divides their range, and both fail loudly if the rounding overshoots;
- `a_reversed_axis_flips_the_value_direction_and_major_unit_places_the_ticks`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4730`) and
  `a_log_axis_places_values_by_their_logarithm`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4704`) — both pin bounds and one pins the unit, so
  neither should notice;
- `degenerate_rects_and_extreme_ranges_stay_finite`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3866`) — the guard that a `log10` on a zero or
  infinite span does not escape.

`crates/pptx-raster/tests/golden/chart.png` is *not* affected: `golden_chart`
(`crates/pptx-raster/tests/golden.rs:346`) hand-builds its primitives and never calls
`plot_chart_into`. `crates/xlsx-render/tests/snapshots/chart_display_list.json` is a pie chart and
carries no tick labels.

The assertion nobody has written yet: given an auto axis over `0..12.3`, every tick label is an
integer and the last one is `>= 12.3`.
