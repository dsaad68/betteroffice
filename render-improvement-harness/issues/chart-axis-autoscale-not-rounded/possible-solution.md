# Possible solution: chart-axis-autoscale-not-rounded

## Approach

Resolve one number — the major unit — before anything else on the axis is decided, then let both
the bounds and the ticks follow from it. Today the unit is implicit (`span / 4`, at
`crates/ooxml-drawingml/src/chart/geometry.rs:1631-1636`) and the bounds are the raw data
(`crates/ooxml-drawingml/src/chart/geometry.rs:1559`); the two are computed in different places
and neither knows about the other.

1. **A nice-number helper.** `{1, 2, 2.5, 5} x 10^k`, rounding to the nearest rather than up —
   rounding up overshoots badly on this deck's numbers (see Risks). Put it next to `axis_ticks`.

2. **A target interval count from the axis extent.** This is the part the report flags as a
   judgement call, and it is what makes the answer match LibreOffice. A vertical value axis stacks
   ~10px labels and fits one every ~28px; a horizontal one lays out `12.3`-wide strings and needs
   ~140px. Three cases pin the constants, and all three must land:

   | axis | extent | target | span | unit | ticks |
   |---|---|---|---|---|---|
   | `stacked-bar` (horizontal, auto) | 979px | 7 | 12.3 | 2 | `0 2 4 6 8 10 12 14` — the reference |
   | `column_chart_emits_background_title_axes_bars_and_legend` (vertical, auto) | 138px | 5 | 20 | 5 | `0 5 10 15 20` — 5 gridlines, so the test's exact count of 7 `PlotOp::Line`s holds |
   | docx snapshots (vertical, pinned `0..25`) | 118px | 4 | 25 | 5 | `0 5 10 15 20 25` — what PowerPoint shows |

   `138 = 200 - 28 - 34` and `118 = 180 - 28 - 34` from
   `crates/ooxml-drawingml/src/chart/geometry.rs:781-795` and `:815-819`; the 979px is measured off
   `render-improvement-harness/decks/stacked-bar/lo-img/01.png`.

3. **Extend only the auto ends.** `value_scale`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:1566-1575`) already knows which ends `c:scaling`
   pinned, because `bounds.min` / `bounds.max` are `Option`. Round the *unpinned* end outward to a
   multiple of the unit and leave a pinned one alone, so
   `axis_bounds_override_the_data_range` (`crates/ooxml-drawingml/src/chart/geometry.rs:3517`)
   still sees `(-10.0, 10.0)` exactly. The unit is picked either way — that is what fixes the docx
   snapshot case, where the bounds are pinned but the labels are still quartered.

4. **Carry the unit on `ValueScale`.** `ValueScale`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:1427`) is already the one thing both the ticks
   and the bar geometry read, so it is the right place. `axis_ticks` then loses its fallback
   entirely: `unit.or(scale.unit)` always resolves, and the existing `c:majorUnit` walk at
   `crates/ooxml-drawingml/src/chart/geometry.rs:1612-1629` generates every tick.

5. **Plumb the extent.** `value_scale(family)` has ten call sites
   (`crates/ooxml-drawingml/src/chart/geometry.rs:1555`, `:1654`, `:2003`, `:2129`, `:2189`,
   `:2351`, `:2407`, `:2483`, `:2616`, `:2747`); all but the `#[cfg(test)] value_range` helper at
   `:1555` have `plot: PlotArea` in scope, so the signature change is mechanical. Pass `plot.h`
   for an upright family and `plot.w` for a transposed one — the same `"bar"` test
   `chart-axis-position-swapped` introduces. Do that issue first if both are in flight, or hard-code
   `plot.h` here and revisit; getting it wrong only picks a slightly-off tick count, not a wrong
   number.

   `scatter_x_scale` (`crates/ooxml-drawingml/src/chart/geometry.rs:2256`) takes the same treatment
   with `plot.w`, since it feeds the same `axis_ticks`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:2316`).

## Sketch

```rust
// crates/ooxml-drawingml/src/chart/geometry.rs

/// The nearest `{1, 2, 2.5, 5} x 10^k` to `rough`, the steps a reader expects
/// an axis to count in.
fn nice_unit(rough: f64) -> f64 {
    if !(rough > 0.0) || !rough.is_finite() {
        return 1.0;
    }
    let pow = 10.0_f64.powf(rough.log10().floor());
    let scaled = rough / pow;
    let nice = if scaled < 1.5 {
        1.0
    } else if scaled < 3.0 {
        2.0
    } else if scaled < 7.0 {
        5.0
    } else {
        10.0
    };
    nice * pow
}

/// Major intervals that fit along `extent` px: a horizontal axis lays its
/// labels end to end and needs far more room per tick than a vertical one.
fn target_intervals(extent: f64, horizontal: bool) -> f64 {
    let per_label = if horizontal { 140.0 } else { 28.0 };
    (extent / per_label).round().clamp(2.0, 12.0)
}

fn value_scale(family: PlotFamily<'_>, extent: f64, horizontal: bool) -> ValueScale {
    // ... stacking / bounds exactly as today, through geometry.rs:1581 ...
    let unit = family
        .axis
        .and_then(|axis| axis.major_unit)
        .filter(|unit| unit.is_finite() && *unit > 0.0)
        .unwrap_or_else(|| nice_unit((max - min) / target_intervals(extent, horizontal)));
    // A log axis keeps its powers-of-base ticks and its raw bounds.
    if log_base.is_none() {
        if bounds.min.is_none() {
            min = (min / unit).floor() * unit;
        }
        if bounds.max.is_none() {
            max = (max / unit).ceil() * unit;
        }
    }
    ValueScale { min, max, unit, /* log_base, reversed, percent as today */ }
}

// axis_ticks: the (0..=4) fallback at geometry.rs:1631-1636 disappears; the
// major-unit walk becomes the only non-log path.
fn axis_ticks(scale: ValueScale, unit: Option<f64>) -> Vec<f64> {
    // ... log branch unchanged (geometry.rs:1599-1611) ...
    let unit = unit
        .filter(|unit| unit.is_finite() && *unit > 0.0)
        .unwrap_or(scale.unit);
    // ... the existing walk at geometry.rs:1615-1628, minus the `ticks.len() >= 2` bail
}
```

## Risks

- **Rounding the unit up instead of to nearest is the trap.** `nice_unit(12.3 / 7) = nice_unit(1.76)`
  must give 2. A ceiling variant gives 2 as well, but on the pinned `0..25` docx axis it turns
  `25 / 4 = 6.25` into 10 (ticks `0 10 20`, dropping the endpoint) where nearest gives 5 (ticks
  `0 5 10 15 20 25`). Both worked cases are in the table above; get one wrong and the other still
  passes.
- **Every chart in three renderers moves.** `plot_chart_into`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:759`) serves
  `crates/pptx-render/src/chart.rs:47`, `crates/xlsx-render/src/chart.rs:498` and
  `crates/docx-layout/src/display_list.rs:7975`. `crates/docx-layout/tests/chart_snapshots.rs`
  regenerates almost wholesale — 70 configurations, 288 tick-label lines, and every gridline and
  bar coordinate that shifts with a new `max`. The test prints its full actual output on failure
  (`crates/docx-layout/tests/chart_snapshots.rs:682-698`); that output is the new `EXPECTED`. Read
  the diff rather than pasting blind: a snapshot that gains a *fractional* label is a bug in the
  change.
- **`an_axis_number_format_reaches_the_tick_labels`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4874`) fails by construction.** It asserts `"75%"`
  on a pinned `0..1` axis, which only exists because 0.75 is the third quarter. With a 0.2 unit the
  labels are `0% 20% 40% 60% 80% 100%`. Re-point it at `"80%"`; the thing it actually tests — that
  `c:numFmt` reaches the tick text — is unchanged.
- **Percent-stacked axes.** `percent_range`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:1507`) works in fractions, so the unit lands near
  0.2 and rounding `1.0` outward is a no-op — but only if `nice_unit` handles sub-1 values, which
  is why it takes `log10().floor()` rather than an integer digit count.
  `percent_stacked_normalizes_every_category`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4409`) asserting `"100%"` is the guard.
- **Log axes must be exempt from the bounds rounding**, or `1..1000` becomes something that is no
  longer a power of the base and `a_log_axis_places_values_by_their_logarithm`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4704`) drifts. The sketch gates on
  `log_base.is_none()`.
- **Degenerate ranges.** A zero or non-finite span reaches `nice_unit` through
  `(max - min) / target`; the guard clause returns 1.0 rather than propagating a NaN unit into
  `axis_ticks`, whose walk would then never terminate on the `value > scale.max` test.
  `degenerate_rects_and_extreme_ranges_stay_finite`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3866`) and `MAX_PLOT_AXIS_TICKS`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:51`) are the two backstops; keep the tick cap.
- **A tiny plot area.** `tiny-rect` and `wide-flat-rect`
  (`crates/docx-layout/tests/chart_snapshots.rs:676-678`) render into 12x8 and 900x26, where
  `plot.h` floors at 24 (`crates/ooxml-drawingml/src/chart/geometry.rs:819`). The
  `clamp(2.0, 12.0)` on the target is what stops those from asking for zero or a hundred ticks.
- Tests to add, in the `crates/ooxml-drawingml/src/chart/geometry.rs` test module: an auto axis
  over `0..12.3` whose every tick label parses as an integer and whose last tick is `>= 12.3`
  (the `stacked-bar` case, and the one assertion nothing covers today); a `nice_unit` unit test
  over the boundaries `1.4 / 1.5 / 2.9 / 3.0 / 6.9 / 7.0` and across a decade; and a case with an
  explicit `c:majorUnit` proving the author's unit still wins over the computed one.

## Effort

Medium. The algorithm is ~40 lines in one file and the plumbing is a mechanical signature change
across ten call sites — but it re-baselines essentially the whole of
`crates/docx-layout/tests/chart_snapshots.rs`, and picking the interval count is a tuning decision
that has to be checked against three worked cases rather than derived from the spec.
