# pptx: Line chart series, axis labels, and noFill axis line all mis-rendered

**Describe the bug**

`minimal-chart/01` is a single line chart on a teal patterned chart space. LibreOffice draws
the white polyline, the white square markers and both sets of white tick labels over that
teal; BetterOffice draws an empty white box with one grey horizontal rule along the bottom
and a broken grey vertical rule on the left (evidence-1.png).

The series, the markers and every tick label are **not** missing. BetterOffice emits all of
them, in the right places, in the right colour — `#FFFFFF`, correctly resolved from
`<a:schemeClr val="bg1"/>`. They are invisible because the chart space behind them is
painted with a hardcoded `#FFFFFF` instead of the file's `a:pattFill`. Repainting
BetterOffice's own display list over the teal ground makes the whole chart appear
(evidence-2.png, third panel).

The spurious vertical stroke is a separate defect: the value axis line is drawn at a
hardcoded `#666666` even though `c:valAx/c:spPr/a:ln` is `noFill`, and its 10px "break" is
the first white square marker painted over it (evidence-3.png).

Seen on 3 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact low, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. minimal-chart/01** reference: teal ground, white line, markers, `0`-`70` and `SUN`-`SAT` labels. Candidate: white box, one grey bottom rule, one broken grey left rule

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-minimal-chart-series-axis-broken/evidence-1.png)

**2. minimal-chart/01** the same candidate display list repainted over the teal ground - line, markers, tick labels, plus a spurious data label per point and a spurious `Series 1` legend key

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-minimal-chart-series-axis-broken/evidence-2.png)

**3. minimal-chart/01** 3x zoom on the top-left corner: LO draws no axis line; BO draws a `#666666` one with a 10px gap, and the gap is exactly where the white `SUN` marker sits

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-minimal-chart-series-axis-broken/evidence-3.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `Simple and minimalistic chart design.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 1

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

### A. The chart space is always painted `#FFFFFF`, so white-on-teal becomes white-on-white

`plot_chart_into` opens every chart with a background rectangle in a constant:

- [`crates/ooxml-drawingml/src/chart/geometry.rs:779`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L779) - `push_rect(ops, x, y, width, height, CHART_BACKGROUND_COLOR)`
- [`crates/ooxml-drawingml/src/chart/geometry.rs:13`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L13) - `pub const CHART_BACKGROUND_COLOR: &str = "#FFFFFF";`

There is nothing for it to read instead. `parse_chart_space`
([`crates/ooxml-drawingml/src/chart/parse.rs:72-138`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L72-L138)) descends into `c:plotArea` and reads
`c:txPr` off the chart space (`parse.rs:133`), but never touches `c:chartSpace/c:spPr`;
`ChartSpace` ([`crates/ooxml-drawingml/src/chart/model.rs:6-24`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/model.rs#L6-L24)) and `PlotChart`
(`geometry.rs:218-233`) have no fill field at all. So even a plain `solidFill` chart-space
background is dropped, and this is the same gap already recorded as section C of
`fill-nonsolid-fill-types-not-resolved` - that issue's `a:pattFill` parser work is
necessary but not sufficient; without a fill field on `ChartSpace` and a read at
`geometry.rs:779` there is nowhere to put the pattern.

The three colours that make the content invisible are all resolved *correctly*:

- Series and marker: `parse_series_color` (`parse.rs:334-340`) runs `first_deep(spPr,
  "solidFill")` and so picks up the `a:solidFill` inside `<a:ln>`; `bg1` resolves through
  the deck theme to `#FFFFFF` ([`crates/pptx-parse/src/chart.rs:64-69`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/chart.rs#L64-L69)).
- Tick labels: `parse_text_properties` (`parse.rs:474-497`) reads the `a:defRPr/a:solidFill`
  on each axis's `c:txPr`, again `bg1` -> `#FFFFFF`.

Both matched LibreOffice. Only the ground is wrong.

### B. Axis lines ignore `c:catAx`/`c:valAx` `c:spPr` entirely

`emit_axes` always strokes the two plot edges at `CHART_AXIS_COLOR` (`#666666`,
`geometry.rs:10`):

- [`crates/ooxml-drawingml/src/chart/geometry.rs:1724-1732`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1724-L1732) - the vertical value-axis line
- [`crates/ooxml-drawingml/src/chart/geometry.rs:1733-1741`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1733-L1741) - the horizontal category-axis line

Neither is gated on anything the file says. `parse_axis`
([`crates/ooxml-drawingml/src/chart/parse.rs:520-564`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L520-L564)) never looks at `c:spPr`, `ChartAxis`
(`model.rs:210-254`) has no line field, and `PlotAxis` (`geometry.rs:275-293`) has none
either. So `<c:valAx><c:spPr><a:ln><a:noFill/></a:ln></c:spPr>` cannot reach the renderer.

The reference is asymmetric in exactly the way the XML is: the category axis declares
`<a:ln w="9525"><a:solidFill><a:schemeClr val="tx1"/ lumMod 15% lumOff 85%>` and LO draws a
light rule along the bottom; the value axis declares `noFill` and LO draws nothing. The
candidate draws both, in the same wrong `#666666`.

### C. The "two disconnected segments" is the first marker overpainting the axis

`emit_line` calls `emit_axes` first (`geometry.rs:2128`) and only then walks the points,
pushing a marker per category (`geometry.rs:2152-2161` -> `push_marker`,
`geometry.rs:2922`, whose `Square` branch is a filled `push_rect` at `geometry.rs:2934`).
`SUN` is the first category and `line_x` (`geometry.rs:2092`) puts it on the plot's left
edge, `x = 96.10` - the same `x` as the value axis line. A 10x10 opaque white rect at
`x` 91.10-101.10, `y` 113.03-123.03 therefore erases 10 rows of the axis line. That is the
whole of finding `minimal-chart/01/4`'s reported break at `y` 0.157-0.171, and it
disappears on its own once B is fixed.

**Suggested fix**

Two independent changes, both of the same shape: carry a shape property that is currently
never parsed from `c:chartSpace`/`c:catAx`/`c:valAx` through the model into the shared plot
geometry, and stop hardcoding a constant there.

**1. Chart-space fill.** Add `fill: Option<ChartFill>` to `ChartSpace`
(`crates/ooxml-drawingml/src/chart/model.rs`) and read `c:chartSpace/c:spPr` in
`parse_chart_space` ([`crates/ooxml-drawingml/src/chart/parse.rs:72`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L72)). The minimum useful
shape is an enum with `None` (an `a:noFill`) and `Solid(String)` — `ChartXml::solid_fill_hex`
already resolves `a:solidFill` through the host theme, so no trait change is needed for
that. `a:pattFill` is a third variant that only becomes reachable once
`fill-nonsolid-fill-types-not-resolved` lands its `pattFill` parse and a `Paint` variant to
carry it; until then, fall back to the pattern's `bgClr` so `minimal-chart` at least gets a
teal ground and the white ink becomes visible. Mirror the field on `PlotChart`
(`geometry.rs:218`) and its `From<&ChartSpace>` impl (`geometry.rs:439`), and make the
opening `push_rect` at `geometry.rs:779` use it, skipping the rect entirely for `noFill`.

**2. Axis line.** Add `line: Option<ChartLine>` to `ChartAxis` (`model.rs:210`) — enough to
distinguish "absent" (draw the current default), "noFill" (draw nothing) and a
`solidFill` colour plus `w` in EMU. Read `c:spPr/a:ln` in `parse_axis` (`parse.rs:520`),
mirror it on `PlotAxis` (`geometry.rs:275`), and gate the two `push_line` calls in
`emit_axes` (`geometry.rs:1724` and `geometry.rs:1733`) on it. The category-axis edge takes
`family.category_axis`'s line, the value-axis edge takes `family.axis`'s; today both use
`CHART_AXIS_COLOR` unconditionally.

`crates/pptx-render/src/chart.rs` needs no structural change for either: `PlotOp::Rect ->
Paint::Solid` (`chart.rs:111`) already carries a solid colour, and a suppressed axis line is
simply an op that is never pushed. It only needs a change if the pattern variant is wired
through, which is the other issue's scope.

Nothing needs to be done about the broken vertical stroke; it is the value axis line with a
marker painted over it, and change 2 removes the line.

```rust
// model.rs
pub enum ChartFill { None, Solid(String) }        // Pattern(..) later
pub struct ChartLine { pub none: bool, pub color: Option<String>, pub width_emu: Option<f64> }

// parse.rs, in parse_chart_space
fill: parse_fill(child(chart_space, "spPr")),

fn parse_fill<E: ChartXml>(properties: Option<&E>) -> Option<ChartFill> {
    let properties = properties?;
    if child(properties, "noFill").is_some() { return Some(ChartFill::None); }
    child(properties, "solidFill").and_then(E::solid_fill_hex).map(ChartFill::Solid)
}

// parse.rs, in parse_axis
line: child(axis, "spPr").and_then(|p| child(p, "ln")).map(|ln| ChartLine {
    none: child(&ln, "noFill").is_some(),
    color: first_deep(ln, "solidFill", 0).and_then(E::solid_fill_hex),
    width_emu: parse_number(ln.attribute(None, "w")),
}),

// geometry.rs, plot_chart_into
match chart.fill {
    Some(PlotFill::None) => {}
    Some(PlotFill::Solid(color)) => push_rect(ops, x, y, width, height, color),
    None => push_rect(ops, x, y, width, height, CHART_BACKGROUND_COLOR),
}

// geometry.rs, emit_axes
if let Some((color, w)) = axis_stroke(family.axis) {
    push_line(ops, plot.x, plot.y, plot.x, plot.y + plot.h, color, w);
}
```

Risks and tests to add:

- `PlotChart`/`PlotAxis` are shared by `crates/xlsx-render/src/chart.rs` and
  `crates/docx-layout/src/display_list.rs` as well as pptx. Both new fields default to
  `None`, so those hosts keep today's behaviour, but their chart golden tests should be run.
- `ChartSpace` and `ChartAxis` are `Serialize`/`Deserialize` and appear in snapshots. Both
  fields need `#[serde(default, skip_serializing_if = "Option::is_none")]` so existing
  fixtures round-trip unchanged.
- Suppressing the chart-space rect for `noFill` means the slide's own background shows
  through. That is correct, but any test asserting "a chart always emits a background rect
  first" will need updating; `crates/pptx-render/src/lib.rs` and the raster golden suite are
  the places to check.
- The pattern fallback (`bgClr` as a flat colour) is deliberately temporary. It should be
  removed, not extended, when the `pattFill` work lands — leaving both would give two
  competing pattern paths.
- Tests to add: a `c:chartSpace/c:spPr` with `solidFill` and with `noFill` in
  `crates/pptx-parse/src/chart.rs`'s parse suite; an axis `a:ln/a:noFill` case in the
  `emit_axes` geometry tests asserting the vertical `push_line` is gone; and a
  `minimal-chart`-shaped raster golden covering white ink over a non-white chart ground.

**How to verify**

1. Re-render with `.venv/bin/python render-improvement-harness/scripts/pipeline.py`
   (or `render_bo.py` + `diff.py`) for `minimal-chart`. `01`'s `fine_pct` is 54.87 today and
   its four hot cells are all in row 2 at 82-99%; after A it should fall below about 10 and
   the row-2 cells with it, since the chart region is 55% of the slide and the ink already
   lands in the right places.
2. Sample `bo-img/01.png` at the chart centre: it must become `#01C4BF`/`#01BABC`, not
   `#FFFFFF`. Column `x=95` must carry no `#666666` run at all once B lands, and the two
   grey rules must become one light bottom rule near `#D9D9D9`.
3. Existing coverage to extend: the chart geometry tests in
   `crates/ooxml-drawingml/src/chart/geometry.rs` (the line-chart cases around lines
   3600-3760 and the marker cases at 4469-4490), the chart-part parse tests in
   [`crates/pptx-parse/src/chart.rs:78-217`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/chart.rs#L78-L217), and the pptx-render chart assertions in
   `crates/pptx-render/src/lib.rs`. None of them currently feeds a `c:chartSpace/c:spPr` or
   an axis `a:noFill`.

**Additional context**

*Also visible once the background is fixed (not this cluster)*

evidence-2's third panel exposes three more defects that the white-on-white currently hides.
None is in this cluster's findings and none should be fixed here:

- **A `Series 1` legend key and swatch**, though the part has no `c:legend`. `has_legend`
  (`geometry.rs:1380-1386`) defaults an absent legend to visible, and the 104px it reserves
  (`geometry.rs:804`) also narrows the plot area. Belongs with
  `chart-legend-and-title-position-wrong`.
- **A data label at every point** (`50 60 40 30 60 30`), though `c:dLbls` sets
  `showVal="0"`. Same defect as `chart-dlbls-shown-when-disabled`.
- **Value ticks `0 15 30 45 60`** where LO picks `0 10 ... 70`. That is
  `chart-axis-autoscale-not-rounded`.

*Not confirmed*

- The series stroke width is hardcoded to `2.0` at `geometry.rs:2150`; the file asks for
  `<a:ln w="41275">`, i.e. 3.25pt (about 4.3px at this scale). `ChartSeries` has no
  stroke-width field to carry it. Real, but invisible in this deck's diff while the
  background is white, and not one of the cluster's findings.
- `c:marker/c:size val="10"` is in points per ECMA-376; `push_marker` consumes it as device
  pixels. LO's markers measure about 12px against BetterOffice's 10px. Sub-3px, so it is a
  suspicion from reading the code rather than something this slide proves.

Related issues found in the same run: `chart-axis-autoscale-not-rounded`, `chart-dlbls-shown-when-disabled`, `chart-legend-and-title-position-wrong`, `fill-nonsolid-fill-types-not-resolved`

Files most likely involved: `crates/ooxml-drawingml/src/chart/geometry.rs`, `crates/ooxml-drawingml/src/chart/parse.rs`, `crates/ooxml-drawingml/src/chart/model.rs`, `crates/pptx-render/src/chart.rs`, `crates/pptx-parse/src/chart.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/chart-minimal-chart-series-axis-broken/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
