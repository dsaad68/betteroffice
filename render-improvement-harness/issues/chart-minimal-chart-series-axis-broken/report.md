---
id: chart-minimal-chart-series-axis-broken
title: Paint the chart space's own fill and honour the axis line's noFill instead of hardcoding white and grey
category: chart
impact: low
effort: medium
confidence: high
status: open
occurrences: 3
decks: [minimal-chart]
findings: [minimal-chart/01/1, minimal-chart/01/2, minimal-chart/01/4]
files: [crates/ooxml-drawingml/src/chart/geometry.rs, crates/ooxml-drawingml/src/chart/parse.rs, crates/ooxml-drawingml/src/chart/model.rs, crates/pptx-render/src/chart.rs, crates/pptx-parse/src/chart.rs]
---

## Symptom

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

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | minimal-chart/01 | reference: teal ground, white line, markers, `0`-`70` and `SUN`-`SAT` labels. Candidate: white box, one grey bottom rule, one broken grey left rule |
| 2 | minimal-chart/01 | the same candidate display list repainted over the teal ground - line, markers, tick labels, plus a spurious data label per point and a spurious `Series 1` legend key |
| 3 | minimal-chart/01 | 3x zoom on the top-left corner: LO draws no axis line; BO draws a `#666666` one with a 10px gap, and the gap is exactly where the white `SUN` marker sits |

The display list BetterOffice produces for the chart part, via the Python binding
(`bindings/python-pptx`, `Presentation.render_slide(0).to_dict()`), abbreviated:

```json
{"kind":"shape","x":54.10,"y":44.57,"w":1171.81,"h":424.76,"geometry":"rect",
 "fill":{"kind":"solid","color":"#FFFFFF"}}                          // chart space
{"kind":"textBox","x":58.10,"y":42.38,"runs":[["60","#FFFFFF"]]}     // value tick label
{"kind":"shape","x":96.10,"y":54.57,"w":0.0,"h":380.76,"geometry":"line",
 "stroke":{"color":"#666666","width":1.0}}                           // value axis line
{"kind":"textBox","x":80.10,"y":434.14,"runs":[["SUN","#FFFFFF"]]}   // category label
{"kind":"shape","x":91.10,"y":113.03,"w":10.0,"h":10.0,"geometry":"rect",
 "fill":{"kind":"solid","color":"#FFFFFF"}}                          // SUN marker
{"kind":"shape","x":96.10,"y":54.57,"w":203.16,"h":63.46,"geometry":"line",
 "stroke":{"color":"#FFFFFF","width":2.0}}                           // series segment
```

Pixel counts back this up. In the chart's bounding box (`x` 54-1226, `y` 45-469) the
candidate is 494154 pure-white pixels plus two grey rules; the reference is 267680 px of
`#01C4BF` and 215372 px of `#01BABC` (the `a:pattFill` `fgClr`/`bgClr`) plus 5284 px of
white ink. Column `x=95` in the candidate is `#C2C2C2` for `y` 54-112 and 123-435 - the
gap `y` 113-122 matches the marker rect at `y` 113.03-123.03 exactly.

## Root cause (confirmed)

### A. The chart space is always painted `#FFFFFF`, so white-on-teal becomes white-on-white

`plot_chart_into` opens every chart with a background rectangle in a constant:

- `crates/ooxml-drawingml/src/chart/geometry.rs:779` - `push_rect(ops, x, y, width, height, CHART_BACKGROUND_COLOR)`
- `crates/ooxml-drawingml/src/chart/geometry.rs:13` - `pub const CHART_BACKGROUND_COLOR: &str = "#FFFFFF";`

There is nothing for it to read instead. `parse_chart_space`
(`crates/ooxml-drawingml/src/chart/parse.rs:72-138`) descends into `c:plotArea` and reads
`c:txPr` off the chart space (`parse.rs:133`), but never touches `c:chartSpace/c:spPr`;
`ChartSpace` (`crates/ooxml-drawingml/src/chart/model.rs:6-24`) and `PlotChart`
(`geometry.rs:218-233`) have no fill field at all. So even a plain `solidFill` chart-space
background is dropped, and this is the same gap already recorded as section C of
`fill-nonsolid-fill-types-not-resolved` - that issue's `a:pattFill` parser work is
necessary but not sufficient; without a fill field on `ChartSpace` and a read at
`geometry.rs:779` there is nowhere to put the pattern.

The three colours that make the content invisible are all resolved *correctly*:

- Series and marker: `parse_series_color` (`parse.rs:334-340`) runs `first_deep(spPr,
  "solidFill")` and so picks up the `a:solidFill` inside `<a:ln>`; `bg1` resolves through
  the deck theme to `#FFFFFF` (`crates/pptx-parse/src/chart.rs:64-69`).
- Tick labels: `parse_text_properties` (`parse.rs:474-497`) reads the `a:defRPr/a:solidFill`
  on each axis's `c:txPr`, again `bg1` -> `#FFFFFF`.

Both matched LibreOffice. Only the ground is wrong.

### B. Axis lines ignore `c:catAx`/`c:valAx` `c:spPr` entirely

`emit_axes` always strokes the two plot edges at `CHART_AXIS_COLOR` (`#666666`,
`geometry.rs:10`):

- `crates/ooxml-drawingml/src/chart/geometry.rs:1724-1732` - the vertical value-axis line
- `crates/ooxml-drawingml/src/chart/geometry.rs:1733-1741` - the horizontal category-axis line

Neither is gated on anything the file says. `parse_axis`
(`crates/ooxml-drawingml/src/chart/parse.rs:520-564`) never looks at `c:spPr`, `ChartAxis`
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

## Also visible once the background is fixed (not this cluster)

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

## Not confirmed

- The series stroke width is hardcoded to `2.0` at `geometry.rs:2150`; the file asks for
  `<a:ln w="41275">`, i.e. 3.25pt (about 4.3px at this scale). `ChartSeries` has no
  stroke-width field to carry it. Real, but invisible in this deck's diff while the
  background is white, and not one of the cluster's findings.
- `c:marker/c:size val="10"` is in points per ECMA-376; `push_marker` consumes it as device
  pixels. LO's markers measure about 12px against BetterOffice's 10px. Sub-3px, so it is a
  suspicion from reading the code rather than something this slide proves.

## Verification

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
   `crates/pptx-parse/src/chart.rs:78-217`, and the pptx-render chart assertions in
   `crates/pptx-render/src/lib.rs`. None of them currently feeds a `c:chartSpace/c:spPr` or
   an axis `a:noFill`.
