# Possible solution: chart-minimal-chart-series-axis-broken

## Approach

Two independent changes, both of the same shape: carry a shape property that is currently
never parsed from `c:chartSpace`/`c:catAx`/`c:valAx` through the model into the shared plot
geometry, and stop hardcoding a constant there.

**1. Chart-space fill.** Add `fill: Option<ChartFill>` to `ChartSpace`
(`crates/ooxml-drawingml/src/chart/model.rs`) and read `c:chartSpace/c:spPr` in
`parse_chart_space` (`crates/ooxml-drawingml/src/chart/parse.rs:72`). The minimum useful
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

## Sketch

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

## Risks

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

## Effort

Medium. Each half is a small, well-understood field added along an existing parse ->
model -> geometry path with no new algorithm, but it touches five files across three crates,
changes two serialized model structs that three renderer hosts share, and its most visible
case (`a:pattFill`) is blocked on `fill-nonsolid-fill-types-not-resolved`.
