# Possible solution: fill-nonsolid-fill-types-not-resolved

Three independent changes, listed in decreasing value per line of code. Ship (1) on its
own — it is a one-line fix that clears four of the six findings.

## 1. Sort gradient stops (easy, fixes typography-trick 01/02/03)

Sort by position at the display-list boundary, in `paint`
(`crates/pptx-render/src/layout.rs:1908-1918`), rather than in the parser. The display
list is the contract both backends read, tiny-skia is order-sensitive while canvas
`addColorStop` silently sorts, so normalising here is what makes the two agree; it also
leaves `pptx-parse`'s model faithful to the document, so `set_fill`
(`crates/pptx-parse/src/write.rs:1052-1067`) does not start reordering `a:gsLst` on save.

```rust
// crates/pptx-render/src/layout.rs, in paint()
let mut stops = gradient
    .stops
    .iter()
    .filter_map(/* unchanged */)
    .collect::<Vec<_>>();
stops.sort_by(|a, b| a.position.total_cmp(&b.position));
```

Belt and braces: `gradient_paint` (`crates/pptx-raster/src/lib.rs:627`) may also sort its
local `colors` vec, so a hand-written or third-party display list cannot reproduce the
tiny-skia pinning bug. That is cheap and independent of the layout change.

## 2. Pattern fills on shapes (medium, fixes cisco-cloud-security/07/5)

- Parse `a:pattFill` in `parse_fill` (`crates/pptx-parse/src/drawing.rs:565`) into a new
  `PatternFill { preset: String, foreground: Option<ColorValue>, background: Option<ColorValue> }`
  on `ShapeFill` (`crates/ooxml-drawingml/src/shape.rs:27-39` is where `GradientFill`
  lives). `write.rs` already lists `pattFill` in `FILL_ELEMENTS`, so the round trip slot
  exists.
- Add a `Paint::Pattern { preset, foreground, background }` variant
  (`crates/pptx-render/src/display_list.rs:24-34`), mirror it in
  `packages/pptx/src/types.ts:205-210`, and bump `CONTRACT_VERSION`
  (`crates/pptx-render/src/display_list.rs:5`).
- Raster: build the preset's tile into a small `Pixmap` (the ECMA-376 presets are all
  expressible on an 8x8 or 24x24 grid) and use `tiny_skia::Pattern` with
  `SpreadMode::Repeat`, returned from `shader_paint`
  (`crates/pptx-raster/src/lib.rs:603-622`). Canvas: draw the same tile to an offscreen
  canvas and `ctx.createPattern(tile, 'repeat')` in `resolvePaint`
  (`packages/pptx/src/render/canvas.ts:177-197`).

```rust
// crates/pptx-raster/src/lib.rs
SlidePaint::Pattern { preset, foreground, background } => {
    let tile = pattern_tile(preset, parse_color(foreground)?, parse_color(background)?)?;
    Ok(Paint {
        shader: Pattern::new(tile.as_ref(), SpreadMode::Repeat,
                             FilterQuality::Nearest, 1.0, Transform::identity()),
        anti_alias: true,
        ..Paint::default()
    })
}
```

Cheaper first cut, if the tile work is too much for one PR: resolve a pattern to
`Paint::Solid` by blending `fgClr` over `bgClr` at the preset's coverage fraction (`pct30`
-> 0.30, `smGrid`/`ltUpDiag` -> their line-density fractions). That gets the *average*
colour right — the cisco band becomes pale green instead of white, the chart space becomes
teal instead of white — with no contract change at all, and can be swapped for real tiles
later. Only 3 distinct presets appear in the whole corpus (`ltUpDiag`, `pct30`, `smGrid`).

## 3. Chart space and plot area fill (medium, needed for minimal-chart/01/3)

Independent of (2) — the chart space has no fill at all today, solid or otherwise.

- Add `fill: Option<PlotFill>` to `ChartSpace`
  (`crates/ooxml-drawingml/src/chart/model.rs:5-24`) and read `c:chartSpace/c:spPr` and
  `c:plotArea/c:spPr` in `parse_chart_space`
  (`crates/ooxml-drawingml/src/chart/parse.rs:72`). The `ChartXml::solid_fill_hex` hook
  (`crates/ooxml-drawingml/src/chart/parse.rs:66-68`) is the existing seam; a
  `pattern_fill` sibling hook keeps the host-specific colour resolution on the host side.
- Emit a background op before the plot ops so it paints under everything. `PlotOp::Rect`
  currently hard-codes a solid `fill: String`
  (`crates/pptx-render/src/chart.rs:111-118`); either widen it or add a
  `PlotOp::Background` carrying the richer paint.

## Risks

- **Contract bump.** A new `Paint` variant means every display-list consumer must handle
  it. `packages/pptx/src/render/canvas.ts` and `png.test.ts` are in-repo; check the wasm
  boundary (`crates/pptx-wasm`) and `bindings/python-pptx` for exhaustive matches.
- **Reordering vs. round trip.** Sorting in `paint` only — not in `parse_gradient_fill` —
  keeps `save`/`set_shape_fill` byte-stable for untouched gradients. Do not "helpfully"
  sort in the parser as well.
- **Chart z-order.** A chart-space background emitted after the series would erase the
  plot; it has to be the first op, and the plot-op budget
  (`MAX_CHART_PRIMITIVES`, `crates/pptx-render/src/layout.rs:37`) must not be able to starve it.
- **Pattern tiles and scale.** The raster runs at `options.scale`; a nearest-neighbour
  tile shader at scale != 1 will alias. Pick the filter quality deliberately and add a
  golden at scale 2.

Tests to add: a display-list test feeding a descending `gsLst` and asserting ascending
stops out (next to `crates/pptx-render/src/lib.rs:485`); a raster golden for a radial
background with reversed stops asserting centre != corner; a canvas-backend parity test in
`packages/pptx/src/render/canvas.test.ts`; a `pptx-parse` test that `a:pattFill` parses
rather than yielding `None`; a chart test that `c:chartSpace/c:spPr` reaches the primitives.

## Effort

**medium** overall — (1) is a one-line, high-confidence fix that resolves four of the six
findings, but (2) and (3) each need a new model field, a display-list contract change and
matching work in both the raster and canvas backends; they are worth splitting into their
own PRs.
