# Possible solution: chart-legend-and-title-position-wrong

## Approach

Two independent changes in `plot_chart_into`
(`crates/ooxml-drawingml/src/chart/geometry.rs:759`), sharing one new idea: text that has to be
centred needs an alignment on the op, because the geometry crate cannot measure a string.

### 1. Reserve the legend band on the right edge, or the right side

Today `legend_w = 104.0` always comes off `plot.w`
(`crates/ooxml-drawingml/src/chart/geometry.rs:804`, `:818`) and `legend_x` is one of two x values
(`crates/ooxml-drawingml/src/chart/geometry.rs:893-897`). Replace both with a single "where does
the legend live" decision that yields a rect, and subtract that rect from the plot on the axis it
actually occupies:

```rust
/// The strip `plot_chart_into` gives the legend, and the side it comes off.
struct LegendBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    horizontal: bool,
}
```

- `"left"` / `"right"` (and the `None` default): as today, `w = 104.0`, `horizontal = false`,
  taken out of `plot.w`.
- `"bottom"` / `"top"`: `w = width - 16.0`, `h = LEGEND_ROW_H` (~22px for one row of
  `CHART_LABEL_SIZE_PX` text plus padding), `horizontal = true`, taken out of `plot.h` — and
  `legend_w` drops to the no-legend `8.0` so the plot gets its width back. A bottom legend goes
  below the 34px category-axis band (`crates/ooxml-drawingml/src/chart/geometry.rs:819`), so
  `plot.h = height - title_h - 34.0 - legend_h`; a top legend goes between the title and the plot,
  so `plot.y = y + title_h + legend_h`.

`has_legend` (`crates/ooxml-drawingml/src/chart/geometry.rs:1380`) still gates all of it.

### 2. Flow the entries horizontally when the band is horizontal

`emit_legend`'s loop only advances in y
(`crates/ooxml-drawingml/src/chart/geometry.rs:3161-3165`). Give it the `LegendBox` and a
horizontal branch that lays the entries out left to right and centres the whole row on the band.
Advance per entry has to be estimated — there are no metrics here — with the same kind of constant
the file already uses for tick centring (`- 16.0` at
`crates/ooxml-drawingml/src/chart/geometry.rs:1692`):

```rust
/// Rough advance of `label`, good enough to centre a legend row: the sans faces
/// the charts use average a little under 0.5 em across mixed-case text.
fn text_advance(label: &str, size_px: f64) -> f64 {
    label.chars().count() as f64 * size_px * 0.52
}
```

Two rows are possible when the entries do not fit (`MAX_LEGEND_ENTRIES` is 8,
`crates/ooxml-drawingml/src/chart/geometry.rs:62`); the simplest correct behaviour is to wrap and
let `LegendBox::h` grow by `LEGEND_ROW_H` per row, which means the band height has to be computed
before the plot rect, i.e. `emit_legend`'s measuring half has to be split out of its drawing half.

### 3. Centre the title

`PlotOp::Text` (`crates/ooxml-drawingml/src/chart/geometry.rs:171-178`) grows an alignment, which
is what the title needs and what the horizontal legend can reuse:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlotTextAlign {
    #[default]
    Start,
    Center,
}
```

Every existing `push_text` call keeps `Start` (add a `push_text_aligned` and leave `push_text`
delegating, so the 14 call sites do not change). The title becomes:

```rust
// geometry.rs:781-794
push_text_aligned(
    ops, title,
    x + 8.0, y + 18.0, (width - 16.0).max(0.0),
    &style, PlotTextAlign::Center,
);
```

with `x` and `width` unchanged — the box is already the full frame minus 8px a side, so centring
inside it lands on the frame centre, x=639 on this deck, which is what the reference does.

Each host resolves it against the `width` it already receives:

- **pptx** is the easy one: `chart_text_primitive`
  (`crates/pptx-render/src/layout.rs:1077`) shapes the run itself and already sums the advances
  into `cursor` (`crates/pptx-render/src/layout.rs:1107`). Sum `shaped` once before the glyph
  loop and offset the start:

  ```rust
  let advance: f32 = shaped.iter().map(|glyph| glyph.x_advance).sum();
  let x = match text.align {
      PlotTextAlign::Center => x + ((safe_geometry(text.width as f32) - advance) / 2.0).max(0.0),
      PlotTextAlign::Start => x,
  };
  ```

  Everything downstream (`run.x`, the glyph cursor, the `TextBox` rect, the `lines` entry) is
  derived from that `x`, and `pptx-raster` paints from the positioned glyphs
  (`crates/pptx-raster/src/font.rs:47-69`), so the shift is the whole fix. Set
  `align: Some(TextAlign::Center)` on the paragraph too
  (`crates/pptx-render/src/layout.rs:1135`) for consumers that re-layout.
- **xlsx** already carries the concept: `DrawCmd::Text` takes an `Align` and its consumer treats
  `Align::Center` as "x is the centre" (`crates/xlsx-render/src/lib.rs:477`,
  `crates/xlsx-render/src/lib.rs:667`), so `crates/xlsx-render/src/chart.rs:810-833` maps
  `Center` to `x + width / 2.0` with `align: Align::Center`.
- **docx** builds a `TextRunPrimitive` with no alignment
  (`crates/docx-layout/src/display_list.rs:8009-8037`). It can stay `Start` initially — a
  left-aligned chart title is what it renders today — as long as the new field is matched
  exhaustively so it is a visible TODO rather than a silent drop.

## Sketch

```rust
// crates/ooxml-drawingml/src/chart/geometry.rs
const LEGEND_ROW_H: f64 = 22.0;
const LEGEND_COL_W: f64 = 104.0;

fn legend_box(chart: &PlotChart<'_>, position: &str, x: f64, y: f64, w: f64, h: f64, title_h: f64)
    -> Option<LegendBox>
{
    if !has_legend(chart) {
        return None;
    }
    Some(match position {
        "bottom" => LegendBox { x: x + 8.0, y: y + h - LEGEND_ROW_H + 6.0, w: w - 16.0, h: LEGEND_ROW_H, horizontal: true },
        "top"    => LegendBox { x: x + 8.0, y: y + title_h + 6.0,          w: w - 16.0, h: LEGEND_ROW_H, horizontal: true },
        "left"   => LegendBox { x: x + 6.0, y: y + title_h + 8.0, w: LEGEND_COL_W - 12.0, h: h - title_h, horizontal: false },
        _        => LegendBox { x: x + w - LEGEND_COL_W + 6.0, y: y + title_h + 8.0, w: LEGEND_COL_W - 12.0, h: h - title_h, horizontal: false },
    })
}

// plot_chart_into, replacing geometry.rs:804-820
let legend = legend_box(chart, legend_position, x, y, width, height, title_h);
let side_w = legend.as_ref().filter(|band| !band.horizontal).map_or(8.0, |_| LEGEND_COL_W);
let band_h = legend.as_ref().filter(|band| band.horizontal).map_or(0.0, |band| band.h);
let plot = PlotArea {
    x: if legend_position == "left" { x + side_w + 42.0 } else { x + 42.0 },
    y: y + title_h + if legend_position == "top" { band_h } else { 0.0 },
    w: (width - 42.0 - side_w - 10.0 - secondary_w).max(24.0),
    h: (height - title_h - 34.0 - band_h).max(24.0),
};

// emit_legend's horizontal branch, replacing geometry.rs:3161-3165
if band.horizontal {
    let gap = 14.0;
    let entry_w = |label: &str| 8.0 + 4.0 + text_advance(label, style.font.size_px);
    let total: f64 = entries.iter().map(|(label, _)| entry_w(label) + gap).sum::<f64>() - gap;
    let mut cursor = band.x + ((band.w - total) / 2.0).max(0.0);
    for (label, color) in &entries {
        push_rect(ops, cursor, band.y + 4.0, 8.0, 8.0, color);
        push_text(ops, label, cursor + 12.0, band.y + 12.0, entry_w(label), style);
        cursor += entry_w(label) + gap;
    }
} else {
    // today's column, unchanged
}
```

## Risks

- **Shared geometry, three renderers.** `plot_chart_into` is called from
  `crates/pptx-render/src/chart.rs:47`, `crates/xlsx-render/src/chart.rs:498` and
  `crates/docx-layout/src/display_list.rs:7975`, and `plot_chart` from
  `crates/ooxml-drawingml/src/chart/parse.rs:965`. Adding a field to `PlotOp::Text` breaks all
  four match arms at compile time, which is the point — a silently ignored alignment would ship
  the bug in two formats out of three.
- **The plot area moves for every bottom-legend chart.** Handing 104px of width back and taking
  ~22px of height changes the aspect of a lot of charts at once, in all three formats. Every chart
  golden that has a legend with `legendPos="b"` will move; `crates/pptx-raster/tests/golden/chart.png`
  (`crates/pptx-raster/tests/golden.rs:346`) needs regenerating, and the diff should be inspected,
  not just accepted.
- **The advance estimate is the weak point.** `text_advance` will drift on long series names and on
  CJK, so the row will be centred approximately and, at 8 entries, may overrun the frame. Clamping
  the row to `band.w` and wrapping is the safe behaviour; centring exactly is not achievable in
  this crate.
- **Title centring changes every chart with a title**, including the ones the harness is not
  looking at. `column_chart_emits_background_title_axes_bars_and_legend`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:3402`) asserts the title op's identity and
  position in `ops`, not its x, so it will keep passing — which means a new assertion is required
  or the regression is invisible to the suite.
- **Interaction with the sibling clusters.** `chart-axis-position-swapped` widens the left gutter
  and moves the value ticks to the bottom band. Both issues touch the same 6 lines of plot-rect
  arithmetic (`crates/ooxml-drawingml/src/chart/geometry.rs:805-820`); doing them in either order
  is fine but doing them concurrently will conflict.
- Tests to add, in the `crates/ooxml-drawingml/src/chart/geometry.rs` test module: a chart with
  `PlotLegend { position: Some("bottom"), .. }` whose legend swatch rects all share one y and have
  strictly increasing x, and whose y is greater than every bar rect's y; the mirror case that
  `Some("right")` and `None` keep today's column; that a bottom legend's `plot.w` is wider than a
  right legend's on the same rect; and a title assertion that a `Center` op's resolved x centres
  the run — that last one belongs on the host side, in `crates/pptx-render/src/layout.rs`, since
  the geometry crate cannot measure.

## Effort

Medium. The legend band and the horizontal flow are contained in two functions of one file, but
the title needs a new field on `PlotOp::Text` and a resolution rule in each of the three hosts,
and the plot-rect change moves every legend-bearing chart in the product, so the golden churn and
the regression assertions are most of the work.
