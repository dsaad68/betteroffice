# pptx: Chart value-axis auto-scale doesn't round to friendly increments

**Describe the bug**

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

Seen on 4 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact low, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. stacked-bar/01** the two charts side by side: `0 2 4 6 8 10 12 14` along the reference's bottom edge against the candidate's `0 3.1 6.2 9.2 12.3`

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-autoscale-not-rounded/evidence-1.png)

**2. stacked-bar/01** 1.7x on the two label sets alone — rounded, step 2, against 12.3 in quarters

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-autoscale-not-rounded/evidence-2.png)

**3. stacked-bar/01** the Category-4 stack in both engines at matched plot width: the reference leaves ~12% headroom past 12.3, the candidate has none because `max == 12.3`

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-autoscale-not-rounded/evidence-3.png)

**4. stacked-bar/04** the same failure on the deck's other slides (04 shown; 02 and 03 differ only in palette)

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-autoscale-not-rounded/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `Stacked Bar Graph That Will Impress Your Clients  Microsoft PowerPoint (PPT) Tutorial.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 2, 3, 4

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(0, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

Two independent gaps, both in `crates/ooxml-drawingml/src/chart/geometry.rs`, and the defect on
this deck needs both fixed.

**1. There is no auto major unit.** `axis_ticks`
([`crates/ooxml-drawingml/src/chart/geometry.rs:1597`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1597)) honours `c:majorUnit` when the file names one
([`crates/ooxml-drawingml/src/chart/geometry.rs:1612`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1612)) and otherwise falls through to a literal
quartering of the range:

```rust
    (0..=4)
        .map(|step| match step {
            4 => scale.max,
            step => scale.min + span * step as f64 / 4.0,
        })
        .collect()
```

([`crates/ooxml-drawingml/src/chart/geometry.rs:1631-1636`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1631-L1636); the doc comment at
[`crates/ooxml-drawingml/src/chart/geometry.rs:1595-1596`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1595-L1596) states the behaviour outright — "at five
even steps otherwise"). Nothing ever asks whether `span / 4` is a number a reader would want to
see. Spreadsheet engines pick a unit from `{1, 2, 2.5, 5} x 10^k` instead.

**2. The auto bounds are the raw data bounds.** `value_scale`
([`crates/ooxml-drawingml/src/chart/geometry.rs:1559`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1559)) takes `min`/`max` straight from
`stacked_totals` / `percent_range` / `plain_range`
([`crates/ooxml-drawingml/src/chart/geometry.rs:1483`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1483), `:1507`, `:1530`), overrides either end that
`c:scaling` pins ([`crates/ooxml-drawingml/src/chart/geometry.rs:1566-1575`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1566-L1575)), and returns. When an
end is *not* pinned it stays on the data value; nothing extends it outward to the next unit.

The arithmetic on this deck matches exactly. The three series sum per category to
`8.7, 8.9, 8.3, 12.3`, so `stacked_totals` returns `(0.0, 12.3)`; `c:scaling` carries only
`<c:orientation val="minMax"/>` in all four of `xml/01/chart-chart1.xml` ...
`xml/04/chart-chart4.xml`, and there is no `c:majorUnit` anywhere in them. The quartering therefore
yields `0, 3.075, 6.15, 9.225, 12.3`, which `format_number`
([`crates/ooxml-drawingml/src/chart/geometry.rs:3169-3175`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3169-L3175)) renders one decimal at a time as
`0, 3.1, 6.2, 9.2, 12.3` — the exact five labels in evidence-2.png.

The same `ValueScale` ([`crates/ooxml-drawingml/src/chart/geometry.rs:1427`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1427)) positions the bars, so
the un-extended `max` is what pins the longest stack to the plot edge in evidence-3.png:
`emit_bar` maps values with `scale.x` ([`crates/ooxml-drawingml/src/chart/geometry.rs:2003`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2003)), and
`ValueScale::fraction` returns exactly 1.0 for `value == max`
([`crates/ooxml-drawingml/src/chart/geometry.rs:1444`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1444)). Rounding the bounds and rounding the ticks
cannot be done separately: they are the same number.

**The product's own baseline shows gap 1 in isolation.** `crates/docx-layout/tests/chart_snapshots.rs`
builds every case from a chart with an *explicit* `0..25` value axis
([`crates/docx-layout/tests/chart_snapshots.rs:140`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/tests/chart_snapshots.rs#L140)) and no major unit, and the committed snapshot
records the quartering:

```
{"baselineY":71,...,"text":"25",...}
{"baselineY":100.5,...,"text":"18.8",...}
{"baselineY":130,...,"text":"12.5",...}
{"baselineY":159.5,...,"text":"6.2",...}
```

([`crates/docx-layout/tests/chart_snapshots.rs:724`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/tests/chart_snapshots.rs#L724), `:726`, `:728`, `:730`). PowerPoint and
LibreOffice both show `0 5 10 15 20 25` there. So the missing auto unit is not specific to auto
bounds — it is wrong on every value axis that does not name `c:majorUnit`, which is most of them.

Reach: `plot_chart_into` ([`crates/ooxml-drawingml/src/chart/geometry.rs:759`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L759)) is the single entry
point for all three renderers — [`crates/pptx-render/src/chart.rs:47`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L47),
[`crates/xlsx-render/src/chart.rs:498`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/xlsx-render/src/chart.rs#L498), [`crates/docx-layout/src/display_list.rs:7975`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/display_list.rs#L7975) — so one
change moves charts in every format.

Not confirmed / judgement calls:

- **How many intervals to aim for.** This is the one thing the evidence does not settle, and it
  decides the answer. The textbook Heckbert "nice numbers" with a fixed 5 ticks gives step 5 on
  this data (`0 5 10 15`), not LibreOffice's step 2. Reproducing `0..14 by 2` needs roughly 7
  intervals, and the reference spends ~979px on that axis — about 122px per label. LibreOffice
  and Excel both size the interval count from the axis extent, which is why a horizontal value axis
  gets more ticks than a vertical one. `axis_ticks` has no access to the plot rectangle today
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:1597`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1597) takes only the scale and the unit), so
  this is the plumbing the fix actually costs. The solution note proposes an extent-derived target;
  the exact divisor is a tuning choice, not a spec value.
- **Whether to extend a pinned end.** The report assumes not: `c:max`/`c:min` are the author's
  numbers and must survive verbatim, which is what `axis_bounds_override_the_data_range`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3517`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3517)) already asserts. Only the *unit* is
  auto-picked when the bounds are pinned. That is how the docx snapshot case above should behave.
- **Log axes.** `axis_ticks` already handles a log axis with powers of the base
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:1599-1611`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1599-L1611)) and must keep that path; nice numbers
  do not apply. No deck in the harness exercises it — `a_log_axis_places_values_by_their_logarithm`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4704`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4704)) is the only coverage.
- **The scatter x axis has the identical defect**, reasoned from the code only, with no deck to
  show it: `scatter_x_scale` ([`crates/ooxml-drawingml/src/chart/geometry.rs:2256`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2256)) derives raw
  bounds the same way and `emit_scatter_x_labels`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:2303`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2303)) ticks them through the same `axis_ticks`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:2316`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2316)). Fixing one should fix both.
- **This cluster is independent of the other three on this deck.** `chart-axis-position-swapped`
  moves these labels to the bottom edge, `chart-dlbls-shown-when-disabled` removes the digits past
  each bar, `chart-legend-and-title-position-wrong` moves the legend. None changes the tick
  *values*. Note the interaction, though: once the value axis runs horizontally, five labels
  `0 / 3.1 / 6.2 / 9.2 / 12.3` sit in one row and can collide, where stacked vertically they did
  not — so the position fix makes this one more visible, not less.

**Suggested fix**

Resolve one number — the major unit — before anything else on the axis is decided, then let both
the bounds and the ticks follow from it. Today the unit is implicit (`span / 4`, at
[`crates/ooxml-drawingml/src/chart/geometry.rs:1631-1636`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1631-L1636)) and the bounds are the raw data
([`crates/ooxml-drawingml/src/chart/geometry.rs:1559`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1559)); the two are computed in different places
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
   [`crates/ooxml-drawingml/src/chart/geometry.rs:781-795`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L781-L795) and `:815-819`; the 979px is measured off
   `render-improvement-harness/decks/stacked-bar/lo-img/01.png`.

3. **Extend only the auto ends.** `value_scale`
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:1566-1575`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1566-L1575)) already knows which ends `c:scaling`
   pinned, because `bounds.min` / `bounds.max` are `Option`. Round the *unpinned* end outward to a
   multiple of the unit and leave a pinned one alone, so
   `axis_bounds_override_the_data_range` ([`crates/ooxml-drawingml/src/chart/geometry.rs:3517`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3517))
   still sees `(-10.0, 10.0)` exactly. The unit is picked either way — that is what fixes the docx
   snapshot case, where the bounds are pinned but the labels are still quartered.

4. **Carry the unit on `ValueScale`.** `ValueScale`
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:1427`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1427)) is already the one thing both the ticks
   and the bar geometry read, so it is the right place. `axis_ticks` then loses its fallback
   entirely: `unit.or(scale.unit)` always resolves, and the existing `c:majorUnit` walk at
   [`crates/ooxml-drawingml/src/chart/geometry.rs:1612-1629`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1612-L1629) generates every tick.

5. **Plumb the extent.** `value_scale(family)` has ten call sites
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:1555`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1555), `:1654`, `:2003`, `:2129`, `:2189`,
   `:2351`, `:2407`, `:2483`, `:2616`, `:2747`); all but the `#[cfg(test)] value_range` helper at
   `:1555` have `plot: PlotArea` in scope, so the signature change is mechanical. Pass `plot.h`
   for an upright family and `plot.w` for a transposed one — the same `"bar"` test
   `chart-axis-position-swapped` introduces. Do that issue first if both are in flight, or hard-code
   `plot.h` here and revisit; getting it wrong only picks a slightly-off tick count, not a wrong
   number.

   `scatter_x_scale` ([`crates/ooxml-drawingml/src/chart/geometry.rs:2256`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2256)) takes the same treatment
   with `plot.w`, since it feeds the same `axis_ticks`
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:2316`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2316)).

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

Risks and tests to add:

- **Rounding the unit up instead of to nearest is the trap.** `nice_unit(12.3 / 7) = nice_unit(1.76)`
  must give 2. A ceiling variant gives 2 as well, but on the pinned `0..25` docx axis it turns
  `25 / 4 = 6.25` into 10 (ticks `0 10 20`, dropping the endpoint) where nearest gives 5 (ticks
  `0 5 10 15 20 25`). Both worked cases are in the table above; get one wrong and the other still
  passes.
- **Every chart in three renderers moves.** `plot_chart_into`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:759`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L759)) serves
  [`crates/pptx-render/src/chart.rs:47`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L47), [`crates/xlsx-render/src/chart.rs:498`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/xlsx-render/src/chart.rs#L498) and
  [`crates/docx-layout/src/display_list.rs:7975`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/display_list.rs#L7975). `crates/docx-layout/tests/chart_snapshots.rs`
  regenerates almost wholesale — 70 configurations, 288 tick-label lines, and every gridline and
  bar coordinate that shifts with a new `max`. The test prints its full actual output on failure
  ([`crates/docx-layout/tests/chart_snapshots.rs:682-698`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/tests/chart_snapshots.rs#L682-L698)); that output is the new `EXPECTED`. Read
  the diff rather than pasting blind: a snapshot that gains a *fractional* label is a bug in the
  change.
- **`an_axis_number_format_reaches_the_tick_labels`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4874`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4874)) fails by construction.** It asserts `"75%"`
  on a pinned `0..1` axis, which only exists because 0.75 is the third quarter. With a 0.2 unit the
  labels are `0% 20% 40% 60% 80% 100%`. Re-point it at `"80%"`; the thing it actually tests — that
  `c:numFmt` reaches the tick text — is unchanged.
- **Percent-stacked axes.** `percent_range`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:1507`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1507)) works in fractions, so the unit lands near
  0.2 and rounding `1.0` outward is a no-op — but only if `nice_unit` handles sub-1 values, which
  is why it takes `log10().floor()` rather than an integer digit count.
  `percent_stacked_normalizes_every_category`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4409`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4409)) asserting `"100%"` is the guard.
- **Log axes must be exempt from the bounds rounding**, or `1..1000` becomes something that is no
  longer a power of the base and `a_log_axis_places_values_by_their_logarithm`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4704`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4704)) drifts. The sketch gates on
  `log_base.is_none()`.
- **Degenerate ranges.** A zero or non-finite span reaches `nice_unit` through
  `(max - min) / target`; the guard clause returns 1.0 rather than propagating a NaN unit into
  `axis_ticks`, whose walk would then never terminate on the `value > scale.max` test.
  `degenerate_rects_and_extreme_ranges_stay_finite`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3866`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3866)) and `MAX_PLOT_AXIS_TICKS`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:51`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L51)) are the two backstops; keep the tick cap.
- **A tiny plot area.** `tiny-rect` and `wide-flat-rect`
  ([`crates/docx-layout/tests/chart_snapshots.rs:676-678`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/tests/chart_snapshots.rs#L676-L678)) render into 12x8 and 900x26, where
  `plot.h` floors at 24 ([`crates/ooxml-drawingml/src/chart/geometry.rs:819`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L819)). The
  `clamp(2.0, 12.0)` on the target is what stops those from asking for zero or a hundred ticks.
- Tests to add, in the `crates/ooxml-drawingml/src/chart/geometry.rs` test module: an auto axis
  over `0..12.3` whose every tick label parses as an integer and whose last tick is `>= 12.3`
  (the `stacked-bar` case, and the one assertion nothing covers today); a `nice_unit` unit test
  over the boundaries `1.4 / 1.5 / 2.9 / 3.0 / 6.9 / 7.0` and across a decade; and a case with an
  explicit `c:majorUnit` proving the author's unit still wins over the computed one.

**How to verify**

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
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4874`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4874)) asserts `"75%"` on a `0..1` axis. `0.75` is
  the third quarter; no nice unit produces it. This one is a direct casualty and needs a different
  assertion.
- `crates/docx-layout/tests/chart_snapshots.rs` — 70 snapshotted configurations, 288 tick-label
  lines, plus every gridline `y` and every bar height that moves with them. The test has no bless
  flag; it panics with the full actual output
  ([`crates/docx-layout/tests/chart_snapshots.rs:682-698`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/tests/chart_snapshots.rs#L682-L698)), which is what gets pasted back into
  `EXPECTED`. This is the bulk of the work and the reason the effort is not `easy`.

Tests that cover the area and should keep passing unchanged — each is a check that the fix stayed
inside its lane:

- `axis_bounds_override_the_data_range`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3517`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3517)) — explicit `c:min`/`c:max` must not be
  rounded outward;
- `column_chart_emits_background_title_axes_bars_and_legend`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3402`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3402)) counts `PlotOp::Line`s exactly (7) on a
  `0..20` auto axis, so it will hold only if the new unit is 5 there — a useful tripwire on the
  interval-count choice;
- `stacked_bars_pile_onto_one_another`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4389`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4389)) asserts the axis reaches `"25"` on totals
  of 25, and `percent_stacked_normalizes_every_category`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4409`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4409)) asserts `"100%"` — both survive any unit
  that divides their range, and both fail loudly if the rounding overshoots;
- `a_reversed_axis_flips_the_value_direction_and_major_unit_places_the_ticks`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4730`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4730)) and
  `a_log_axis_places_values_by_their_logarithm`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4704`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4704)) — both pin bounds and one pins the unit, so
  neither should notice;
- `degenerate_rects_and_extreme_ranges_stay_finite`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3866`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3866)) — the guard that a `log10` on a zero or
  infinite span does not escape.

`crates/pptx-raster/tests/golden/chart.png` is *not* affected: `golden_chart`
([`crates/pptx-raster/tests/golden.rs:346`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/tests/golden.rs#L346)) hand-builds its primitives and never calls
`plot_chart_into`. `crates/xlsx-render/tests/snapshots/chart_display_list.json` is a pie chart and
carries no tick labels.

The assertion nobody has written yet: given an auto axis over `0..12.3`, every tick label is an
integer and the last one is `>= 12.3`.

**Additional context**

none.

Related issues found in the same run: `chart-axis-position-swapped`, `chart-dlbls-shown-when-disabled`, `chart-legend-and-title-position-wrong`

Files most likely involved: `crates/ooxml-drawingml/src/chart/geometry.rs`, `crates/docx-layout/tests/chart_snapshots.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-autoscale-not-rounded/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
