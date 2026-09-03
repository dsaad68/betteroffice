---
id: chart-legend-and-title-position-wrong
title: Honour c:legendPos for top and bottom legends, and centre the chart title
category: chart
impact: low
effort: medium
confidence: high
status: open
occurrences: 5
decks: [stacked-bar]
findings: [stacked-bar/01/3, stacked-bar/02/4, stacked-bar/03/4, stacked-bar/04/4, stacked-bar/04/5]
files: [crates/ooxml-drawingml/src/chart/geometry.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/chart.rs, crates/xlsx-render/src/chart.rs, crates/docx-layout/src/display_list.rs]
---

## Symptom

The chart declares `<c:legendPos val="b"/>` and `<c:overlay val="0"/>`, so the legend belongs in a
horizontal row under the plot area. The candidate instead stacks the three entries as a vertical
column in the top-right corner of the chart frame, on top of the plot, where it overlaps the first
bar and the title row (evidence-1.png, evidence-2.png).

The title has no manual `c:layout`, so it should auto-centre over the frame. LibreOffice puts
`INDUSTRY TRENDS` ink at x 489-790 - centre 639, which is the frame centre. The candidate starts
the ink at x=83, flush against the frame's left edge (evidence-3.png).

Both defects are one layout block emitting fixed positions, and both repeat unchanged on all four
slides of `stacked-bar`, which are the same chart in four colourways (evidence-4.png).

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | stacked-bar/04 | the whole chart frame in both renders: reference has a centred title and a legend row under the plot; candidate has a left-flush title and a legend column over the top-right of the plot |
| 2 | stacked-bar/04 | the two legends at full frame width - reference at y 564-580 centred on x=639, candidate at y 135-180 in a column at x=1104 |
| 3 | stacked-bar/04 | the two titles at full frame width - reference ink centred at x=639, candidate ink starting at x=83 |
| 4 | stacked-bar/01 | the same placement on another slide of the deck |

## Root cause (confirmed)

`c:legendPos` is parsed and then only half-read: the layout has a left branch and an
everything-else branch, so `"bottom"` and `"top"` both fall through to the right-hand column.
The title has no alignment concept at all - it is pushed at a fixed offset from the frame's left
edge.

**Parse is fine.** `parse_legend` maps `c:legendPos` `l`/`r`/`t`/`b` to
`"left"`/`"right"`/`"top"`/`"bottom"`
(`crates/ooxml-drawingml/src/chart/parse.rs:504-517`), stores it on `ChartLegend::position`
(`crates/ooxml-drawingml/src/chart/model.rs:29`) and `PlotChart::from` copies it onto
`PlotLegend::position` (`crates/ooxml-drawingml/src/chart/geometry.rs:444-447`, field at
`crates/ooxml-drawingml/src/chart/geometry.rs:242`). The value reaches layout intact.

**Layout reads it twice, and both reads are binary.** In `plot_chart_into`:

```rust
let legend_position = chart.legend.as_ref()
    .and_then(|legend| legend.position).unwrap_or("right");   // geometry.rs:799-803
let legend_w = if has_legend(chart) { 104.0 } else { 8.0 };   // geometry.rs:804
let plot_x = if legend_position == "left" {                   // geometry.rs:805-809
    x + legend_w + 42.0
} else {
    x + 42.0
};
let plot = PlotArea {
    // ...
    w: (width - 42.0 - legend_w - 10.0 - secondary_w).max(24.0),  // geometry.rs:818
    h: (height - title_h - 34.0).max(24.0),                       // geometry.rs:819
};
// ...
let legend_x = if legend_position == "left" {                 // geometry.rs:893-897
    x + 6.0
} else {
    x + width - legend_w + 6.0
};
emit_legend(ops, chart, scan, legend_x, y + title_h + 8.0, legend_w - 12.0, legend_style);
                                                              // geometry.rs:898-906
```

`"bottom"` takes the `else` on both, so the legend lands at the top-right. The reserved space is
wrong in the same way: 104px always comes off the plot *width*
(`crates/ooxml-drawingml/src/chart/geometry.rs:818`), never off its height, so a bottom legend gets
no vertical band of its own and has to sit inside the plot. The only height reserved below the plot
is the flat 34px for the category-axis row
(`crates/ooxml-drawingml/src/chart/geometry.rs:819`).

**`emit_legend` can only draw a column.** The entry loop advances only in y:

```rust
for (i, (label, color)) in entries.iter().enumerate() {
    let yy = y + i as f64 * 15.0;                             // geometry.rs:3162
    push_rect(ops, x, yy, 8.0, 8.0, color);                   // geometry.rs:3163
    push_text(ops, label, x + 12.0, yy + 8.0, width - 12.0, style);  // geometry.rs:3164
}
```

There is no horizontal-flow branch and no per-entry width, so even a correct `legend_x`/`legend_y`
for a bottom legend would still produce a stack. Entry count is capped at 8
(`crates/ooxml-drawingml/src/chart/geometry.rs:62`).

The arithmetic matches the pixels exactly. The chart frame on slide 04 is
`<a:off x="740229" y="943429"/><a:ext cx="10711543" cy="4586514"/>`
(`render-improvement-harness/decks/stacked-bar/xml/04/slide.xml`), i.e. x=77.7, y=99.0, w=1124.5 at
1280x720. That gives `legend_x = 77.7 + 1124.5 - 104 + 6 = 1104.2` and
`legend_y = 99.0 + 28 + 8 = 135.0`; the measured yellow swatch in `bo-img/04.png` is at
x 1104-1111, y 135-142, and the next two swatches sit 15px apart at y 150 and 165.

**The title.** It is pushed once, at a constant x, and the band it reserves is a constant too:

```rust
let title_h = if let Some(title) = chart.title.filter(|s| !s.is_empty()) {
    push_text(ops, title, x + 8.0, y + 18.0, (width - 16.0).max(0.0), /* style */);
    28.0                                                      // geometry.rs:781-794
} else {
    10.0
};
```

`x + 8.0 = 85.7` against measured ink starting at x=83 (the glyph's left side bearing accounts for
the difference). `PlotOp::Text` carries `x`, `baseline_y` and `width` but no alignment
(`crates/ooxml-drawingml/src/chart/geometry.rs:171-178`), and every host lays the run out from `x`
rightwards: `chart_text_primitive` starts its glyph cursor at `x` and hard-codes
`align: Some(TextAlign::Left)` (`crates/pptx-render/src/layout.rs:1094`,
`crates/pptx-render/src/layout.rs:1096-1108`, `crates/pptx-render/src/layout.rs:1135`), xlsx emits
`align: Align::Left` (`crates/xlsx-render/src/chart.rs:830`) and docx emits a plain positioned run
(`crates/docx-layout/src/display_list.rs:8009-8022`). So `width` is a bounding hint, not a box the
text is aligned inside - nothing in the pipeline can centre the title today.

Not confirmed / judgement calls:

- **Where a bottom legend's height comes from.** PowerPoint sizes the legend band from the entry
  metrics; the geometry crate has no font metrics (there is no text-width helper anywhere in
  `geometry.rs`), so any bottom-legend band will be an estimate from entry count and
  `style.size_px`. The reference band here is ~16px of ink at y 564-580, ~28px including padding.
  This is reasoned from the code, not measured against a spec.
- **Centring without measuring text.** Same constraint: either `PlotOp::Text` grows an alignment
  field that each host resolves against the `width` it is already given (all three hosts already
  have an align concept at the point they consume it), or `geometry.rs` estimates the advance
  width. The solution note prefers the first; that is a design choice, not something the evidence
  settles.
- **`legendPos="t"` and `"tr"`.** No deck in the harness uses them. `parse_legend` maps only
  `l`/`r`/`t`/`b` and drops anything else to `None`
  (`crates/ooxml-drawingml/src/chart/parse.rs:506-511`), so `tr` already degrades to the default
  right column. Whether `"top"` should also push the plot down past the title is untested here.
- **`c:overlay`.** `<c:overlay val="0"/>` is present on both the legend and the title in this deck
  and is never parsed - grep for `overlay` across `crates/ooxml-drawingml` returns nothing. With
  `val="0"` the correct behaviour is what a non-overlapping layout already does, so it does not
  change this fix; a deck with `val="1"` would need it.
- **The other defects in the same evidence.** The vertical value axis
  (`chart-axis-position-swapped`), reversed categories (`chart-category-order-reversed`), unrounded
  ticks (`chart-axis-autoscale-not-rounded`), stray end-of-bar labels
  (`chart-dlbls-shown-when-disabled`) and the missing `spc="300"` tracking on the title run
  (`text-run-props-spc-ignored`) are separate clusters and will all still show after this fix.

## Verification

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py stacked-bar
.venv/bin/python render-improvement-harness/scripts/diff.py stacked-bar
```

All four slides sit at `fine_pct` 14.46 with hot cells `r2c2` 37.0, `r3c2` 36.7, `r3c3` 30.2,
`r2c3` 27.4 (`render-improvement-harness/decks/stacked-bar/diff-summary.json`). The legend and
title are a few hundred pixels of ink on a 1280x720 frame, so the headline number will barely move;
the checks are positional:

- no legend op may be emitted with x > `plot.x + plot.w` when `legendPos` is `b` or `t` - the
  swatches must leave the x=1104 column;
- the legend swatches must share one y and differ in x, i.e. a row not a column, centred near the
  plot's horizontal centre (x=639 in the reference, evidence-2.png);
- no legend op may fall inside the plot rect, so the top bar is no longer overpainted;
- the title run's centre must land within a few px of the frame centre, x=639 (evidence-3.png);
- the four category rows must not shift vertically by more than the new bottom band, and the plot
  must not lose width - a bottom legend should hand its 104px back to `plot.w`.

Existing tests that cover this area and must keep passing:

- `column_chart_emits_background_title_axes_bars_and_legend`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3402`) asserts the title is `ops[1]` and that a
  `North` text op exists; it does not assert where either lands, so it survives a position change
  but pins the emit order;
- `an_of_pie_group_draws_wedges_and_a_per_slice_legend`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3492`),
  `combo_plot_groups_drive_the_label_and_the_legend`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3588`) and
  `a_legend_key_draws_a_swatch_beside_its_label`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4947`) cover legend entry construction, which this
  fix must not touch;
- `crates/xlsx-render/tests/chart_render.rs:17` builds a `ChartLegend` with
  `position: Some("right")`, and `crates/pptx-render/src/chart.rs:438` /
  `crates/pptx-render/src/chart.rs:470` plot every chart type into primitives - both exercise the
  host side of any `PlotOp::Text` signature change;
- the raster golden `crates/pptx-raster/tests/golden/chart.png`
  (`crates/pptx-raster/tests/golden.rs:346`) will need regenerating if the title moves.

No test asserts a legend's position, only its contents; and no test asserts a title's x. Both
assertions are new.
