# pptx: Chart value axis c:axPos not honored — axes swapped

**Describe the bug**

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

Seen on 4 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. stacked-bar/01** reference vs candidate over the whole axis region: `Category 1`-`Category 4` spelled out on the left with ticks `0`-`14` under the plot, against the candidate's single column of alternating `Categ` / value ticks and a bare bottom edge

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-position-swapped/evidence-1.png)

**2. stacked-bar/01** 2x zoom on the left gutter: the value ticks sit between the category rows, and every category name is cut at the plot edge where the bar paints over it

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-position-swapped/evidence-2.png)

**3. stacked-bar/01** 1.4x zoom of the bottom band: the reference's `0 2 4 6 8 10 12 14` row against the candidate's empty one

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-position-swapped/evidence-3.png)

**4. stacked-bar/04** the same failure on the deck's other slides (04 shown; 02 and 03 are the same chart recoloured)

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-position-swapped/evidence-4.png)

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

`c:axPos` is parsed, carried all the way into the plot model, and then never read.

- `parse_axis` maps `c:axPos` to `ChartAxis::position` as `"left" | "right" | "top" | "bottom"`
  ([`crates/ooxml-drawingml/src/chart/parse.rs:536`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L536)), and `axis_list` collects every `catAx` /
  `dateAx` / `valAx` / `serAx` under the plot area
  ([`crates/ooxml-drawingml/src/chart/parse.rs:117`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L117)).
- `plot_axis_from_model` copies it onto `PlotAxis::position`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:548`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L548), field at
  [`crates/ooxml-drawingml/src/chart/geometry.rs:288`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L288)).
- Nothing reads it. Grepping `.position` across `geometry.rs` returns only that assignment, plus
  data-label positions (`PlotDataLabels::position`,
  [`crates/ooxml-drawingml/src/chart/geometry.rs:362`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L362)), the legend position
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:242`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L242), read at
  [`crates/ooxml-drawingml/src/chart/geometry.rs:799`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L799)) and `Iterator::position` calls. The axis
  field is dead data.

The value axis is therefore always drawn as if it were vertical. `emit_axes`
([`crates/ooxml-drawingml/src/chart/geometry.rs:1649`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1649)) hard-codes one orientation:

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
([`crates/ooxml-drawingml/src/chart/geometry.rs:1661-1665`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1661-L1665),
[`crates/ooxml-drawingml/src/chart/geometry.rs:1700`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1700)), the value-axis title goes above-left at
`plot.x - 38.0, plot.y - 5.0` ([`crates/ooxml-drawingml/src/chart/geometry.rs:1742`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1742)) and the
category-axis title below the plot ([`crates/ooxml-drawingml/src/chart/geometry.rs:1757`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1757)) - all four
placements assume value-vertical / category-horizontal.

`emit_bar` already knows the family is transposed: `emit_family` dispatches `"bar"` with
`horizontal = true` and everything else with `false`
([`crates/ooxml-drawingml/src/chart/geometry.rs:1312-1313`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1312-L1313), fed by `plot_type_for`'s `barDir`
check at [`crates/ooxml-drawingml/src/chart/parse.rs:584`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L584)). It uses that flag for the bars
(`scale.x` instead of `scale.y`, [`crates/ooxml-drawingml/src/chart/geometry.rs:2043-2046`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2043-L2046)) and for
the category labels, which it moves to the left gutter
([`crates/ooxml-drawingml/src/chart/geometry.rs:2013-2021`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2013-L2021)). But it calls
`emit_axes(ops, family, plot)` unconditionally and without the flag
([`crates/ooxml-drawingml/src/chart/geometry.rs:2002`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2002)), so the value axis stays vertical while the
bars and categories transpose. That single omission is the whole defect.

The collision is exact, not approximate: the value tick labels at
[`crates/ooxml-drawingml/src/chart/geometry.rs:1669`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1669) and the horizontal-bar category labels at
[`crates/ooxml-drawingml/src/chart/geometry.rs:2017`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2017) both evaluate to `plot.x - 38.0`. Measured on
`bo-img/01.png`, every text run in the left gutter starts at x=80, i.e. `plot.x = 118`.

The `Categ` truncation is a paint-order effect, not a clip. `push_text` truncates only at 120
chars ([`crates/ooxml-drawingml/src/chart/geometry.rs:1349`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1349)) and `chart_text_primitive` widens the
box to the measured run rather than clipping it
([`crates/pptx-render/src/layout.rs:1134`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1134): `w: safe_geometry(text.width as f32).max(width)`). The
label is emitted first and the bar rect for the same category immediately after
([`crates/ooxml-drawingml/src/chart/geometry.rs:2012`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2012) then
[`crates/ooxml-drawingml/src/chart/geometry.rs:2046`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2046)), so the opaque bar covers everything past
`plot.x`. Widening the gutter is therefore part of the fix, not a separate cosmetic issue: the plot
rect reserves a fixed 42px on the left ([`crates/ooxml-drawingml/src/chart/geometry.rs:805-818`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L805-L818))
regardless of chart type, and `Category 4` needs ~70px.

The property is reachable on this deck's code path, so a fix keyed on `axPos` will actually see a
value. The `c:barChart` carries both `c:axId`s, `parse_plot_group` collects them
([`crates/ooxml-drawingml/src/chart/parse.rs:616`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L616)), and `group_value_axes` /
`group_category_axis` resolve them to the two definitions
([`crates/ooxml-drawingml/src/chart/geometry.rs:1071`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1071),
[`crates/ooxml-drawingml/src/chart/geometry.rs:1107`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1107)). For `stacked-bar` that yields
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
  edge ([`crates/ooxml-drawingml/src/chart/geometry.rs:1661-1667`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1661-L1667),
  [`crates/ooxml-drawingml/src/chart/geometry.rs:1712-1723`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1712-L1723)); transposed it belongs on the top. No
  deck here has a secondary axis on a bar chart, so this is reasoned from the code only.
- **Gridline direction.** The value gridlines would also have to become vertical. Invisible in
  this evidence: `c:valAx` here declares no `c:majorGridlines`, so `major_grid` is false
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:1657`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1657)) and none are drawn in either render.
- **Category order** (`stacked-bar/02/3`, `stacked-bar/03/3`, `stacked-bar/04/1`) is the separate
  `chart-category-order-reversed` cluster, and the legend/title placement visible in evidence-4 is
  `chart-legend-and-title-position-wrong`. The tick values themselves (`3.1`, `6.2`, ... instead of
  `2`, `4`, ...) are `chart-axis-autoscale-not-rounded`. None of the three is fixed by this issue,
  and all three will still show in a re-render.

**Suggested fix**

Teach `emit_axes` the one thing `emit_bar` already knows: on a `"bar"` family the value axis is
horizontal and the category axis vertical. Everything else follows the conventions the file
already uses elsewhere — `emit_scatter_x_labels`
([`crates/ooxml-drawingml/src/chart/geometry.rs:2303`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2303)) is a value axis drawn along the bottom
today, so the transposed branch can copy its placement (`scale.x(plot, value) - 16.0`, baseline
`plot.y + plot.h + 14.0`, width 32) rather than invent one.

1. **Decide the orientation once.** Add a method on `PlotFamily`
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:991`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L991)) next to `stacking()`:

   ```rust
   /// A horizontal bar family plots values along x, so its value axis runs
   /// along the bottom edge and its category axis down the left.
   fn transposed(&self) -> bool {
       self.chart_type == "bar"
   }
   ```

   Use it at [`crates/ooxml-drawingml/src/chart/geometry.rs:1312-1313`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1312-L1313) in place of the literal
   `true`/`false`, so `emit_family` and `emit_axes` cannot disagree.

2. **Honour `axPos` for the side, not the orientation.** `PlotAxis::position`
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:288`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L288)) holds `"left" | "right" | "top" |
   "bottom"` after [`crates/ooxml-drawingml/src/chart/parse.rs:536`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L536) — not the raw `l`/`r`/`t`/`b`,
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

3. **Branch `emit_axes`** ([`crates/ooxml-drawingml/src/chart/geometry.rs:1649`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1649)). Take a
   `transposed: bool` parameter, pass `family.transposed()` from `emit_bar`
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:2002`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2002)) and `false` from the five other call
   sites ([`crates/ooxml-drawingml/src/chart/geometry.rs:2128`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2128), `:2188`, `:2350`, `:2406`,
   `:2615` — line, area, scatter, bubble, stock, none of which transpose). Four things swap:
   tick position (`scale.x` for `scale.y`), gridline direction, tick-label placement, and which
   plot edge carries the tick marks and the axis line. The two axis titles at
   [`crates/ooxml-drawingml/src/chart/geometry.rs:1742`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1742) and `:1757` swap with them — no rotation
   is needed in either orientation, which is what `PlotOp::Text` supports
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:233`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L233)).

4. **Size the gutters from the chart.** `plot_chart_into` reserves a flat 42px on the left and
   34px at the bottom ([`crates/ooxml-drawingml/src/chart/geometry.rs:805-819`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L805-L819)). A transposed
   chart needs the wider column on the left for category names — the reference spends ~70px on
   `Category 4` — and the bottom band it already has is enough for the ticks. Add a
   `has_transposed_family(chart)` helper alongside `secondary_value_axis`
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:1124`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1124)), which checks
   `group.chart_type.unwrap_or(chart.chart_type) == "bar"` over `chart.plot_groups` (falling back
   to `chart.chart_type` when there are no groups), and widen the left gutter to ~76px when it is
   true.

5. **Spend the wider gutter.** The horizontal category label at
   [`crates/ooxml-drawingml/src/chart/geometry.rs:2013-2021`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2013-L2021) hard-codes `plot.x - 38.0` and width
   `36.0`. Make both derive from the same gutter constant so the label no longer runs under the
   bars — the `Categ` clipping in evidence-2.png is the bar rect painting over the overflow, so
   the gutter is what fixes it.

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

Risks and tests to add:

- **Shared geometry, three renderers.** `plot_chart_into` is called from
  [`crates/pptx-render/src/chart.rs:47`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L47), [`crates/xlsx-render/src/chart.rs:498`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/xlsx-render/src/chart.rs#L498) and
  [`crates/docx-layout/src/display_list.rs:7975`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/display_list.rs#L7975). Every horizontal bar chart in every format moves;
  column, line, area, scatter, bubble, stock, pie must not. Step 3's `false` at the five
  non-bar call sites is what guarantees that, and
  `column_chart_emits_background_title_axes_bars_and_legend`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3402`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3402)) — which counts `PlotOp::Line`s exactly —
  is the tripwire.
- **The gutter change is not confined to bar charts if written carelessly.** `plot.w` is computed
  from the same constant, so a gutter that widens unconditionally shrinks every plot area in the
  product and moves every chart golden. Gate it on `has_transposed_family`.
- **Tick-label centring is an approximation.** `PlotOp::Text` is left-aligned at `x` with no
  measurement available in the geometry, so `- 16.0` centres a two- or three-character tick and
  drifts on long ones. That is the convention `emit_scatter_x_labels` already ships with
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:2323`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2323)); matching it keeps the two bottom-axis
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
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3454`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3454)) extension that checks *where* the two
  titles land for `"bar"`, not just that they exist.

**How to verify**

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
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3402`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3402)) asserts exactly 7 `PlotOp::Line`s for a
  column chart - the column path must be untouched;
- `axis_titles_draw_beside_the_axes_they_name`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:3454`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L3454)) already loops over `"bar"` but only checks
  that the titles exist, not where they land;
- `stacked_bars_pile_onto_one_another` ([`crates/ooxml-drawingml/src/chart/geometry.rs:4389`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4389)),
  `gap_width_and_overlap_size_and_place_the_bars`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4426`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4426)),
  `a_reversed_category_axis_draws_the_categories_from_the_far_end`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4831`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4831)),
  `tick_marks_draw_only_when_the_axis_names_them`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4803`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4803)),
  `gridlines_follow_the_axis_that_declares_them`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:4757`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L4757));
- consumer-side smoke tests `every_parsed_chart_type_plots_into_primitives` and
  `wedge_families_draw_closed_paths_and_flat_families_draw_rectangles`
  ([`crates/pptx-render/src/chart.rs:438`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L438), [`crates/pptx-render/src/chart.rs:470`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L470)), plus the
  `chart` raster golden (`crates/pptx-raster/tests/golden/chart.png`, driven by
  [`crates/pptx-raster/tests/golden.rs:346`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/tests/golden.rs#L346)).

No test asserts where a bar chart's value ticks are drawn; that assertion is the new one.

**Additional context**

none.

Related issues found in the same run: `chart-axis-autoscale-not-rounded`, `chart-category-order-reversed`, `chart-legend-and-title-position-wrong`

Files most likely involved: `crates/ooxml-drawingml/src/chart/geometry.rs`, `crates/pptx-render/src/chart.rs`, `crates/xlsx-render/src/chart.rs`, `crates/docx-layout/src/display_list.rs`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/chart-axis-position-swapped/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
