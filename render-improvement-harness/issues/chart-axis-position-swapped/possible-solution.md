# Possible solution: chart-axis-position-swapped

## Approach

Teach `emit_axes` the one thing `emit_bar` already knows: on a `"bar"` family the value axis is
horizontal and the category axis vertical. Everything else follows the conventions the file
already uses elsewhere — `emit_scatter_x_labels`
(`crates/ooxml-drawingml/src/chart/geometry.rs:2303`) is a value axis drawn along the bottom
today, so the transposed branch can copy its placement (`scale.x(plot, value) - 16.0`, baseline
`plot.y + plot.h + 14.0`, width 32) rather than invent one.

1. **Decide the orientation once.** Add a method on `PlotFamily`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:991`) next to `stacking()`:

   ```rust
   /// A horizontal bar family plots values along x, so its value axis runs
   /// along the bottom edge and its category axis down the left.
   fn transposed(&self) -> bool {
       self.chart_type == "bar"
   }
   ```

   Use it at `crates/ooxml-drawingml/src/chart/geometry.rs:1312-1313` in place of the literal
   `true`/`false`, so `emit_family` and `emit_axes` cannot disagree.

2. **Honour `axPos` for the side, not the orientation.** `PlotAxis::position`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:288`) holds `"left" | "right" | "top" |
   "bottom"` after `crates/ooxml-drawingml/src/chart/parse.rs:536` — not the raw `l`/`r`/`t`/`b`,
   which is the easy mistake here. Fold it into the existing `secondary` test so a value axis on
   the far edge and a secondary axis take the same path:

   ```rust
   /// The value axis sits on the far edge: the top of a transposed plot, the
   /// right of an upright one.
   fn far_side(family: PlotFamily<'_>, transposed: bool) -> bool {
       let far = if transposed { "top" } else { "right" };
       family.axis.and_then(|axis| axis.position) == Some(far)
   }
   ```

   The orientation itself stays keyed on the family, for the reason in the report: `axPos` alone
   would let a malformed deck transpose the axes without transposing the bars.

3. **Branch `emit_axes`** (`crates/ooxml-drawingml/src/chart/geometry.rs:1649`). Take a
   `transposed: bool` parameter, pass `family.transposed()` from `emit_bar`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:2002`) and `false` from the five other call
   sites (`crates/ooxml-drawingml/src/chart/geometry.rs:2128`, `:2188`, `:2350`, `:2406`,
   `:2615` — line, area, scatter, bubble, stock, none of which transpose). Four things swap:
   tick position (`scale.x` for `scale.y`), gridline direction, tick-label placement, and which
   plot edge carries the tick marks and the axis line. The two axis titles at
   `crates/ooxml-drawingml/src/chart/geometry.rs:1742` and `:1757` swap with them — no rotation
   is needed in either orientation, which is what `PlotOp::Text` supports
   (`crates/ooxml-drawingml/src/chart/geometry.rs:233`).

4. **Size the gutters from the chart.** `plot_chart_into` reserves a flat 42px on the left and
   34px at the bottom (`crates/ooxml-drawingml/src/chart/geometry.rs:805-819`). A transposed
   chart needs the wider column on the left for category names — the reference spends ~70px on
   `Category 4` — and the bottom band it already has is enough for the ticks. Add a
   `has_transposed_family(chart)` helper alongside `secondary_value_axis`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:1124`), which checks
   `group.chart_type.unwrap_or(chart.chart_type) == "bar"` over `chart.plot_groups` (falling back
   to `chart.chart_type` when there are no groups), and widen the left gutter to ~76px when it is
   true.

5. **Spend the wider gutter.** The horizontal category label at
   `crates/ooxml-drawingml/src/chart/geometry.rs:2013-2021` hard-codes `plot.x - 38.0` and width
   `36.0`. Make both derive from the same gutter constant so the label no longer runs under the
   bars — the `Categ` clipping in evidence-2.png is the bar rect painting over the overflow, so
   the gutter is what fixes it.

## Sketch

```rust
// crates/ooxml-drawingml/src/chart/geometry.rs
const AXIS_GUTTER: f64 = 42.0;
const CATEGORY_GUTTER: f64 = 76.0;   // a horizontal bar chart's left label column

fn emit_axes<S: PlotSink + ?Sized>(
    ops: &mut Emitter<'_, S>,
    family: PlotFamily<'_>,
    plot: PlotArea,
    transposed: bool,
) {
    // ... scale, hidden, grids, number_format, tick_style as today ...
    let far = family.secondary || far_side(family, transposed);
    for value in axis_ticks(scale, axis.and_then(|axis| axis.major_unit)) {
        if ops.exhausted() {
            return;
        }
        if transposed {
            let x = scale.x(plot, value);
            if major_grid {
                push_line(ops, x, plot.y, x, plot.y + plot.h, CHART_GRID_COLOR, 0.5);
            }
            if hidden {
                continue;
            }
            // the same placement emit_scatter_x_labels already uses (geometry.rs:2320)
            let baseline = if far { plot.y - 6.0 } else { plot.y + plot.h + 14.0 };
            push_text(ops, &scale.format(value, number_format), x - 16.0, baseline, 32.0, tick_style);
            if let Some((outer, inner)) = tick_extents(axis.and_then(|axis| axis.major_tick_mark)) {
                let (edge, outward) = if far { (plot.y, -1.0) } else { (plot.y + plot.h, 1.0) };
                push_line(ops, x, edge + outward * outer, x, edge - outward * inner, CHART_AXIS_COLOR, 1.0);
            }
        } else {
            // today's body, unchanged
        }
    }
    // frame lines: both edges as today; the axis titles swap places when `transposed`
}

// plot_chart_into, replacing the literal 42.0 at geometry.rs:805-818
let gutter = if has_transposed_family(chart) { CATEGORY_GUTTER } else { AXIS_GUTTER };
let plot_x = if legend_position == "left" { x + legend_w + gutter } else { x + gutter };
let plot = PlotArea {
    x: plot_x,
    y: y + title_h,
    w: (width - gutter - legend_w - 10.0 - secondary_w).max(24.0),
    h: (height - title_h - 34.0).max(24.0),
};

// emit_bar's horizontal category label (geometry.rs:2013)
push_text(ops, &label, plot.x - gutter + 4.0, plot.y + slot + bands.slot * 0.55, gutter - 8.0, category_style);
```

`emit_bar` needs the gutter it was laid out with; either recompute it from `family.transposed()`
or carry it on `PlotArea` — the latter keeps one source of truth and is worth the extra field.

## Risks

- **Shared geometry, three renderers.** `plot_chart_into` is called from
  `crates/pptx-render/src/chart.rs:47`, `crates/xlsx-render/src/chart.rs:498` and
  `crates/docx-layout/src/display_list.rs:7975`. Every horizontal bar chart in every format moves;
  column, line, area, scatter, bubble, stock, pie must not. Step 3's `false` at the five
  non-bar call sites is what guarantees that, and
  `column_chart_emits_background_title_axes_bars_and_legend`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3402`) — which counts `PlotOp::Line`s exactly —
  is the tripwire.
- **The gutter change is not confined to bar charts if written carelessly.** `plot.w` is computed
  from the same constant, so a gutter that widens unconditionally shrinks every plot area in the
  product and moves every chart golden. Gate it on `has_transposed_family`.
- **Tick-label centring is an approximation.** `PlotOp::Text` is left-aligned at `x` with no
  measurement available in the geometry, so `- 16.0` centres a two- or three-character tick and
  drifts on long ones. That is the convention `emit_scatter_x_labels` already ships with
  (`crates/ooxml-drawingml/src/chart/geometry.rs:2323`); matching it keeps the two bottom-axis
  paths consistent rather than adding a second rule.
- **Overlap with the sibling clusters.** A transposed axis makes
  `chart-axis-autoscale-not-rounded` more visible, not less: `0 / 3.1 / 6.2 / 9.2 / 12.3` spread
  along the bottom will now collide horizontally where stacked vertically they did not. Fixing
  the autoscale afterwards is what makes the bottom row read like the reference.
- **Combo charts.** A `c:barChart` with `barDir="bar"` grouped with a `c:lineChart` against the
  same axes would transpose one family and not the other. The code cannot render that correctly
  either way today; keep the per-family decision and leave it out of scope.
- Tests to add, in the `crates/ooxml-drawingml/src/chart/geometry.rs` test module: a bar-family
  chart whose value tick labels have distinct y from every category label and sit below
  `plot.y + plot.h`; the mirror assertion that a column chart's tick labels keep their current x;
  a `majorTickMark` assertion that the marks follow the bottom edge when transposed; and an
  `axis_titles_draw_beside_the_axes_they_name`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3454`) extension that checks *where* the two
  titles land for `"bar"`, not just that they exist.

## Effort

Medium. One file and roughly one new branch, with the placement rules already established by
`emit_scatter_x_labels` — but it changes a plot-area constant that every chart in three renderers
shares, so the work is mostly in gating the change to bar families and proving the other eleven
chart types did not move.
