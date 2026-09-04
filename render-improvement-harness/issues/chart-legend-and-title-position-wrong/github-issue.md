# pptx: Chart legend position and title alignment not honored

**Describe the bug**

The chart declares `<c:legendPos val="b"/>` and `<c:overlay val="0"/>`, so the legend belongs in a
horizontal row under the plot area. The candidate instead stacks the three entries as a vertical
column in the top-right corner of the chart frame, on top of the plot, where it overlaps the first
bar and the title row (evidence-1.png, evidence-2.png).

The title has no manual `c:layout`, so it should auto-centre over the frame. LibreOffice puts
`INDUSTRY TRENDS` ink at x 489-790 - centre 639, which is the frame centre. The candidate starts
the ink at x=83, flush against the frame's left edge (evidence-3.png).

Both defects are one layout block emitting fixed positions, and both repeat unchanged on all four
slides of `stacked-bar`, which are the same chart in four colourways (evidence-4.png).

Seen on 5 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact low, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. stacked-bar/04** the whole chart frame in both renders: reference has a centred title and a legend row under the plot; candidate has a left-flush title and a legend column over the top-right of the plot

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-legend-and-title-position-wrong/evidence-1.png)

**2. stacked-bar/04** the two legends at full frame width - reference at y 564-580 centred on x=639, candidate at y 135-180 in a column at x=1104

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-legend-and-title-position-wrong/evidence-2.png)

**3. stacked-bar/04** the two titles at full frame width - reference ink centred at x=639, candidate ink starting at x=83

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-legend-and-title-position-wrong/evidence-3.png)

**4. stacked-bar/01** the same placement on another slide of the deck

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-legend-and-title-position-wrong/evidence-4.png)

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

`c:legendPos` is parsed and then only half-read: the layout has a left branch and an
everything-else branch, so `"bottom"` and `"top"` both fall through to the right-hand column.
The title has no alignment concept at all - it is pushed at a fixed offset from the frame's left
edge.

**Parse is fine.** `parse_legend` maps `c:legendPos` `l`/`r`/`t`/`b` to
`"left"`/`"right"`/`"top"`/`"bottom"`
([`crates/ooxml-drawingml/src/chart/parse.rs:504-517`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L504-L517)), stores it on `ChartLegend::position`
([`crates/ooxml-drawingml/src/chart/model.rs:29`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/model.rs#L29)) and `PlotChart::from` copies it onto
`PlotLegend::position` ([`crates/ooxml-drawingml/src/chart/geometry.rs:444-447`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L444-L447), field at
[`crates/ooxml-drawingml/src/chart/geometry.rs:242`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L242)). The value reaches layout intact.

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
([`crates/ooxml-drawingml/src/chart/geometry.rs:818`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L818)), never off its height, so a bottom legend gets
no vertical band of its own and has to sit inside the plot. The only height reserved below the plot
is the flat 34px for the category-axis row
([`crates/ooxml-drawingml/src/chart/geometry.rs:819`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L819)).

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
([`crates/ooxml-drawingml/src/chart/geometry.rs:62`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L62)).

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
([`crates/ooxml-drawingml/src/chart/geometry.rs:171-178`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L171-L178)), and every host lays the run out from `x`
rightwards: `chart_text_primitive` starts its glyph cursor at `x` and hard-codes
`align: Some(TextAlign::Left)` ([`crates/pptx-render/src/layout.rs:1094`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1094),
[`crates/pptx-render/src/layout.rs:1096-1108`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1096-L1108), [`crates/pptx-render/src/layout.rs:1135`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1135)), xlsx emits
`align: Align::Left` ([`crates/xlsx-render/src/chart.rs:830`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/xlsx-render/src/chart.rs#L830)) and docx emits a plain positioned run
([`crates/docx-layout/src/display_list.rs:8009-8022`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/display_list.rs#L8009-L8022)). So `width` is a bounding hint, not a box the
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
  ([`crates/ooxml-drawingml/src/chart/parse.rs:506-511`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L506-L511)), so `tr` already degrades to the default
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

**Suggested fix**

Two independent changes in `plot_chart_into`
([`crates/ooxml-drawingml/src/chart/geometry.rs:759`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L759)), sharing one new idea: text that has to be
centred needs an alignment on the op, because the geometry crate cannot measure a string.

### 1. Reserve the legend band on the right edge, or the right side

Today `legend_w = 104.0` always comes off `plot.w`
([`crates/ooxml-drawingml/src/chart/geometry.rs:804`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L804), `:818`) and `legend_x` is one of two x values
([`crates/ooxml-drawingml/src/chart/geometry.rs:893-897`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L893-L897)). Replace both with a single "where does
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
  below the 34px category-axis band ([`crates/ooxml-drawingml/src/chart/geometry.rs:819`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L819)), so
  `plot.h = height - title_h - 34.0 - legend_h`; a top legend goes between the title and the plot,
  so `plot.y = y + title_h + legend_h`.

`has_legend` ([`crates/ooxml-drawingml/src/chart/geometry.rs:1380`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1380)) still gates all of it.

### 2. Flow the entries horizontally when the band is horizontal

`emit_legend`'s loop only advances in y
([`crates/ooxml-drawingml/src/chart/geometry.rs:3161-3165`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3161-L3165)). Give it the `LegendBox` and a
horizontal branch that lays the entries out left to right and centres the whole row on the band.
Advance per entry has to be estimated — there are no metrics here — with the same kind of constant
the file already uses for tick centring (`- 16.0` at
[`crates/ooxml-drawingml/src/chart/geometry.rs:1692`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1692)):

```rust
/// Rough advance of `label`, good enough to centre a legend row: the sans faces
/// the charts use average a little under 0.5 em across mixed-case text.
fn text_advance(label: &str, size_px: f64) -> f64 {
    label.chars().count() as f64 * size_px * 0.52
}
```

Two rows are possible when the entries do not fit (`MAX_LEGEND_ENTRIES` is 8,
[`crates/ooxml-drawingml/src/chart/geometry.rs:62`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L62)); the simplest correct behaviour is to wrap and
let `LegendBox::h` grow by `LEGEND_ROW_H` per row, which means the band height has to be computed
before the plot rect, i.e. `emit_legend`'s measuring half has to be split out of its drawing half.

### 3. Centre the title

`PlotOp::Text` ([`crates/ooxml-drawingml/src/chart/geometry.rs:171-178`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L171-L178)) grows an alignment, which
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
  ([`crates/pptx-render/src/layout.rs:1077`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1077)) shapes the run itself and already sums the advances
  into `cursor` ([`crates/pptx-render/src/layout.rs:1107`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1107)). Sum `shaped` once before the glyph
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
  ([`crates/pptx-raster/src/font.rs:47-69`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/font.rs#L47-L69)), so the shift is the whole fix. Set
  `align: Some(TextAlign::Center)` on the paragraph too
  ([`crates/pptx-render/src/layout.rs:1135`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1135)) for consumers that re-layout.
- **xlsx** already carries the concept: `DrawCmd::Text` takes an `Align` and its consumer treats
  `Align::Center` as "x is the centre" ([`crates/xlsx-render/src/lib.rs:477`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/xlsx-render/src/lib.rs#L477),
  [`crates/xlsx-render/src/lib.rs:667`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/xlsx-render/src/lib.rs#L667)), so [`crates/xlsx-render/src/chart.rs:810-833`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/xlsx-render/src/chart.rs#L810-L833) maps
  `Center` to `x + width / 2.0` with `align: Align::Center`.
- **docx** builds a `TextRunPrimitive` with no alignment
  ([`crates/docx-layout/src/display_list.rs:8009-8037`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/display_list.rs#L8009-L8037)). It can stay `Start` initially — a
  left-aligned chart title is what it renders today — as long as the new field is matched
  exhaustively so it is a visible TODO rather than a silent drop.

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

Risks and tests to add:

- **Shared geometry, three renderers.** `plot_chart_into` is called from
  [`crates/pptx-render/src/chart.rs:47`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L47), [`crates/xlsx-render/src/chart.rs:498`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/xlsx-render/src/chart.rs#L498) and
  [`crates/docx-layout/src/display_list.rs:7975`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/display_list.rs#L7975), and `plot_chart` from
  [`crates/ooxml-drawingml/src/chart/parse.rs:965`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L965). Adding a field to `PlotOp::Text` breaks all
  four match arms at compile time, which is the point — a silently ignored alignment would ship
  the bug in two formats out of three.
- **The plot area moves for every bottom-legend chart.** Handing 104px of width back and taking
  ~22px of height changes the aspect of a lot of charts at once, in all three formats. Every chart
  golden that has a legend with `legendPos="b"` will move; `crates/pptx-raster/tests/golden/chart.png`
  ([`crates/pptx-raster/tests/golden.rs:346`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/tests/golden.rs#L346)) needs regenerating, and the diff should be inspected,
  not just accepted.
- **The advance estimate is the weak point.** `text_advance` will drift on long series names and on
  CJK, so the row will be centred approximately and, at 8 entries, may overrun the frame. Clamping
  the row to `band.w` and wrapping is the safe behaviour; centring exactly is not achievable in
  this crate.
- **Title centring changes every chart with a title**, including the ones the harness is not
  looking at. `column_chart_emits_background_title_axes_bars_and_legend`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3402`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3402)) asserts the title op's identity and
  position in `ops`, not its x, so it will keep passing — which means a new assertion is required
  or the regression is invisible to the suite.
- **Interaction with the sibling clusters.** `chart-axis-position-swapped` widens the left gutter
  and moves the value ticks to the bottom band. Both issues touch the same 6 lines of plot-rect
  arithmetic ([`crates/ooxml-drawingml/src/chart/geometry.rs:805-820`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L805-L820)); doing them in either order
  is fine but doing them concurrently will conflict.
- Tests to add, in the `crates/ooxml-drawingml/src/chart/geometry.rs` test module: a chart with
  `PlotLegend { position: Some("bottom"), .. }` whose legend swatch rects all share one y and have
  strictly increasing x, and whose y is greater than every bar rect's y; the mirror case that
  `Some("right")` and `None` keep today's column; that a bottom legend's `plot.w` is wider than a
  right legend's on the same rect; and a title assertion that a `Center` op's resolved x centres
  the run — that last one belongs on the host side, in `crates/pptx-render/src/layout.rs`, since
  the geometry crate cannot measure.

**How to verify**

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
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3402`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3402)) asserts the title is `ops[1]` and that a
  `North` text op exists; it does not assert where either lands, so it survives a position change
  but pins the emit order;
- `an_of_pie_group_draws_wedges_and_a_per_slice_legend`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3492`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3492)),
  `combo_plot_groups_drive_the_label_and_the_legend`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3588`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3588)) and
  `a_legend_key_draws_a_swatch_beside_its_label`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4947`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4947)) cover legend entry construction, which this
  fix must not touch;
- [`crates/xlsx-render/tests/chart_render.rs:17`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/xlsx-render/tests/chart_render.rs#L17) builds a `ChartLegend` with
  `position: Some("right")`, and [`crates/pptx-render/src/chart.rs:438`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L438) /
  [`crates/pptx-render/src/chart.rs:470`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L470) plot every chart type into primitives - both exercise the
  host side of any `PlotOp::Text` signature change;
- the raster golden `crates/pptx-raster/tests/golden/chart.png`
  ([`crates/pptx-raster/tests/golden.rs:346`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/tests/golden.rs#L346)) will need regenerating if the title moves.

No test asserts a legend's position, only its contents; and no test asserts a title's x. Both
assertions are new.

**Additional context**

none.

Related issues found in the same run: `chart-axis-autoscale-not-rounded`, `chart-axis-position-swapped`, `chart-category-order-reversed`, `chart-dlbls-shown-when-disabled`, `text-run-props-spc-ignored`

Files most likely involved: `crates/ooxml-drawingml/src/chart/geometry.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/chart.rs`, `crates/xlsx-render/src/chart.rs`, `crates/docx-layout/src/display_list.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/chart-legend-and-title-position-wrong/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
