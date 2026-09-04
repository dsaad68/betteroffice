# pptx: Non-solid fill types (radial gradFill, pattFill) collapse to a flat fill

**Describe the bug**

Two unrelated defects hide behind the same visual: a fill that should have structure is
painted as one flat colour.

1. **Radial slide backgrounds flatten to the outer stop.** All three `typography-trick`
   slides declare `<a:gradFill><a:path path="circle">` backgrounds. LibreOffice draws the
   glow; BetterOffice draws a single colour that is *exactly* the `pos="100000"` stop
   (evidence-1, evidence-2). This is not a "radial is unimplemented" bug — radial gradients
   are wired end to end — it is a stop-ordering bug, see below.
2. **`a:pattFill` is never parsed.** The `pct30` dot pattern on the "Analyze & Control"
   band in `cisco-cloud-security/07` disappears (evidence-3), and the `smGrid` teal
   pattern on the `minimal-chart` chart space disappears (evidence-4).

`cisco-cloud-security/03/2` is in this cluster but is really the alpha bug: its `a:lin`
gradient has stops in ascending order and both stops are the same RGB, differing only in
`a:alpha` — it belongs with `fill-alpha-modifier-ignored`, not with the two defects below. See "Not this cluster".

Seen on 6 slides across 3 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. typography-trick/03** reference has a blue radial glow at the centre (`accent1` lumMod 50%, the `pos="0"` stop); candidate is flat `#222A35`, the `pos="100000"` stop

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-nonsolid-fill-types-not-resolved/evidence-1.png)

**2. typography-trick/02** same failure in greyscale: reference centre `#404040`, corners `#262626`; candidate is `#262626` everywhere

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-nonsolid-fill-types-not-resolved/evidence-2.png)

**3. cisco-cloud-security/07** reference's "Analyze & Control" band carries the `pct30` cross-hatch; candidate's band is flat white

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-nonsolid-fill-types-not-resolved/evidence-3.png)

**4. minimal-chart/01** reference chart space is filled with the teal `smGrid` pattern; candidate's chart space is unfilled white

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-nonsolid-fill-types-not-resolved/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 3, 7
- `Simple and minimalistic chart design.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 1
- `Trick To Create Beautiful Typography in Microsoft Office PowerPoint PPT.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 2, 3

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(2, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

### A. Gradient stops are emitted in document order and never sorted

`parse_gradient_fill` collects `a:gs` children in document order
([`crates/pptx-parse/src/drawing.rs:594`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L594)) and `paint` maps them straight into the display
list without sorting ([`crates/pptx-render/src/layout.rs:1908`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1908)). All three
`typography-trick` backgrounds list the `pos="100000"` stop *first*:

```xml
<a:gsLst>
  <a:gs pos="100000"><a:schemeClr val="tx1"><a:lumMod val="85000"/><a:lumOff val="15000"/></a:schemeClr></a:gs>
  <a:gs pos="0"><a:schemeClr val="tx1"><a:lumMod val="75000"/><a:lumOff val="25000"/></a:schemeClr></a:gs>
</a:gsLst>
<a:path path="circle"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path>
```

The display list BetterOffice produces for that slide, dumped through the Python binding,
keeps that order and resolves both colours correctly:

```json
{"kind":"gradient","gradientType":"radial",
 "stops":[{"position":1.0,"color":"#262626"},{"position":0.0,"color":"#404040"}]}
```

`gradient_paint` ([`crates/pptx-raster/src/lib.rs:627`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L627)) hands those stops to tiny-skia
unchanged. `tiny_skia::Gradient::new` brackets the list with dummy stops and then *pins
positions monotonically*
(`~/.cargo/registry/src/index.crates.io-*/tiny-skia-0.12.0/src/shaders/gradient.rs:78-93`,
`stops[i].position.get().bound(prev, 1.0)`). For `[1.0 -> #262626, 0.0 -> #404040]` that
yields positions `[0, 1, 1, 1]` with colours `[#262626, #262626, #404040, #404040]`: the
whole 0..1 range is `#262626`. That is exactly the flat colour observed, for all three
slides.

The web backend does *not* have this bug — [`packages/pptx/src/render/canvas.ts:197`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L197) uses
`addColorStop`, and the canvas spec sorts stops by offset — so this is also a
raster/canvas parity divergence, not only a raster defect.

Scope beyond this cluster: 15 out-of-order `a:gsLst` elements across 9 parts in the
harness corpus (`cisco-cloud-security/07`, `/09`, `rollout-plan` layouts 05/06/09,
`triangles-corporate/01`, all three `typography-trick` slides), against 690 already-sorted
ones. Fixing the ordering fixes those too.

### B. `a:pattFill` is not in the parser at all

`parse_fill` ([`crates/pptx-parse/src/drawing.rs:565`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L565)) recognises only `noFill`,
`solidFill`, `gradFill` and `blipFill`. There is no `pattFill` branch anywhere in the pptx
crates — the only occurrences of the string are `docx-parse` and the writer's
`FILL_ELEMENTS` round-trip list at [`crates/pptx-parse/src/write.rs:1000`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1000). A pattern-filled
shape therefore parses to `fill: None` and, at [`crates/pptx-render/src/layout.rs:384-388`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L384-L388),
is drawn unfilled. There is no `Paint` variant to carry a pattern either
([`crates/pptx-render/src/display_list.rs:24-34`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L24-L34): `Solid` and `Gradient` only).

Corpus counts: `ltUpDiag` x3 and `pct30` x1 in `cisco-cloud-security/07`, `smGrid` x1 in
`minimal-chart`'s chart part.

### C. The chart space has no fill of any kind (minimal-chart/01/3 needs this too)

`minimal-chart`'s pattern sits on `c:chartSpace/c:spPr`. `parse_chart_space`
([`crates/ooxml-drawingml/src/chart/parse.rs:72`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L72)) never reads `c:spPr` for the chart space
or the plot area, and `ChartSpace` ([`crates/ooxml-drawingml/src/chart/model.rs:5-24`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/model.rs#L5-L24)) has
no fill field — the parser only ever looks for `solidFill` under a series, data point,
marker or run. The chart sink can only emit solid rectangles anyway
([`crates/pptx-render/src/chart.rs:111-118`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L111-L118), `PlotOp::Rect { fill }` -> `Paint::Solid`). So
even a plain `solidFill` chart-space background is dropped today; fixing B alone will not
make evidence-4 correct.

**Suggested fix**



*1. Sort gradient stops (easy, fixes typography-trick 01/02/03)*

Sort by position at the display-list boundary, in `paint`
([`crates/pptx-render/src/layout.rs:1908-1918`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1908-L1918)), rather than in the parser. The display
list is the contract both backends read, tiny-skia is order-sensitive while canvas
`addColorStop` silently sorts, so normalising here is what makes the two agree; it also
leaves `pptx-parse`'s model faithful to the document, so `set_fill`
([`crates/pptx-parse/src/write.rs:1052-1067`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1052-L1067)) does not start reordering `a:gsLst` on save.

```rust
// crates/pptx-render/src/layout.rs, in paint()
let mut stops = gradient
    .stops
    .iter()
    .filter_map(/* unchanged */)
    .collect::<Vec<_>>();
stops.sort_by(|a, b| a.position.total_cmp(&b.position));
```

Belt and braces: `gradient_paint` ([`crates/pptx-raster/src/lib.rs:627`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L627)) may also sort its
local `colors` vec, so a hand-written or third-party display list cannot reproduce the
tiny-skia pinning bug. That is cheap and independent of the layout change.

*2. Pattern fills on shapes (medium, fixes cisco-cloud-security/07/5)*

- Parse `a:pattFill` in `parse_fill` ([`crates/pptx-parse/src/drawing.rs:565`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L565)) into a new
  `PatternFill { preset: String, foreground: Option<ColorValue>, background: Option<ColorValue> }`
  on `ShapeFill` ([`crates/ooxml-drawingml/src/shape.rs:27-39`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/shape.rs#L27-L39) is where `GradientFill`
  lives). `write.rs` already lists `pattFill` in `FILL_ELEMENTS`, so the round trip slot
  exists.
- Add a `Paint::Pattern { preset, foreground, background }` variant
  ([`crates/pptx-render/src/display_list.rs:24-34`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L24-L34)), mirror it in
  [`packages/pptx/src/types.ts:205-210`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L205-L210), and bump `CONTRACT_VERSION`
  ([`crates/pptx-render/src/display_list.rs:5`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L5)).
- Raster: build the preset's tile into a small `Pixmap` (the ECMA-376 presets are all
  expressible on an 8x8 or 24x24 grid) and use `tiny_skia::Pattern` with
  `SpreadMode::Repeat`, returned from `shader_paint`
  ([`crates/pptx-raster/src/lib.rs:603-622`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L603-L622)). Canvas: draw the same tile to an offscreen
  canvas and `ctx.createPattern(tile, 'repeat')` in `resolvePaint`
  ([`packages/pptx/src/render/canvas.ts:177-197`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L177-L197)).

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

*3. Chart space and plot area fill (medium, needed for minimal-chart/01/3)*

Independent of (2) — the chart space has no fill at all today, solid or otherwise.

- Add `fill: Option<PlotFill>` to `ChartSpace`
  ([`crates/ooxml-drawingml/src/chart/model.rs:5-24`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/model.rs#L5-L24)) and read `c:chartSpace/c:spPr` and
  `c:plotArea/c:spPr` in `parse_chart_space`
  ([`crates/ooxml-drawingml/src/chart/parse.rs:72`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L72)). The `ChartXml::solid_fill_hex` hook
  ([`crates/ooxml-drawingml/src/chart/parse.rs:66-68`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L66-L68)) is the existing seam; a
  `pattern_fill` sibling hook keeps the host-specific colour resolution on the host side.
- Emit a background op before the plot ops so it paints under everything. `PlotOp::Rect`
  currently hard-codes a solid `fill: String`
  ([`crates/pptx-render/src/chart.rs:111-118`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L111-L118)); either widen it or add a
  `PlotOp::Background` carrying the richer paint.

Risks and tests to add:

- **Contract bump.** A new `Paint` variant means every display-list consumer must handle
  it. `packages/pptx/src/render/canvas.ts` and `png.test.ts` are in-repo; check the wasm
  boundary (`crates/pptx-wasm`) and `bindings/python-pptx` for exhaustive matches.
- **Reordering vs. round trip.** Sorting in `paint` only — not in `parse_gradient_fill` —
  keeps `save`/`set_shape_fill` byte-stable for untouched gradients. Do not "helpfully"
  sort in the parser as well.
- **Chart z-order.** A chart-space background emitted after the series would erase the
  plot; it has to be the first op, and the plot-op budget
  (`MAX_CHART_PRIMITIVES`, [`crates/pptx-render/src/layout.rs:37`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L37)) must not be able to starve it.
- **Pattern tiles and scale.** The raster runs at `options.scale`; a nearest-neighbour
  tile shader at scale != 1 will alias. Pick the filter quality deliberately and add a
  golden at scale 2.

Tests to add: a display-list test feeding a descending `gsLst` and asserting ascending
stops out (next to [`crates/pptx-render/src/lib.rs:485`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L485)); a raster golden for a radial
background with reversed stops asserting centre != corner; a canvas-backend parity test in
`packages/pptx/src/render/canvas.test.ts`; a `pptx-parse` test that `a:pattFill` parses
rather than yielding `None`; a chart test that `c:chartSpace/c:spPr` reaches the primitives.

**How to verify**

1. `.venv/bin/python render-improvement-harness/scripts/pipeline.py` (or `render_bo.py` +
   `diff.py`) for `typography-trick`. After the stop-order fix, all three slides' `bo-img`
   centre pixel must match the `pos="0"` stop, not the `pos="100000"` one: `(255,255,255)`
   for 01, `(64,64,64)` for 02, `(32,56,100)` for 03, with a smooth falloff to the corner
   values already matching. Slide 02's `fine_pct` should fall from 3.54 toward ~2, slide
   03's from 6.52 toward ~4 (the wordmark, a different cluster, keeps the rest).
2. Re-render `cisco-cloud-security/07` and `minimal-chart/01` after the pattern work; the
   band and the chart space must stop being white.
3. Existing coverage to extend: the display-list gradient assertion at
   [`crates/pptx-render/src/lib.rs:489-495`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L489-L495), the raster golden suite under
   `crates/pptx-raster/tests/golden.rs`, and `packages/pptx/src/render/canvas.test.ts` for
   backend parity. None of them currently feeds a descending `gsLst`.

**Additional context**

*Not this cluster*

- `cisco-cloud-security/03/2` — an `a:lin` gradient whose two stops are the same resolved
  RGB and differ only in `a:alpha` (42000 -> 0). `resolve_color_value_to_hex_with_theme`
  ([`crates/ooxml-drawingml/src/color.rs:61-87`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L61-L87)) returns opaque `#RRGGBB` and drops alpha,
  so the "gradient" is a flat opaque wash. The sibling resolver
  `resolve_color_value_to_rgba_hex` ([`crates/ooxml-drawingml/src/color.rs:91`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L91)) already
  exists and `pptx-raster`'s `parse_hex_color` already accepts 8-digit hex
  ([`crates/pptx-raster/src/lib.rs:780-796`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L780-L796)). It shares its root cause with
  `fill-alpha-modifier-ignored`, whose fix resolves it; nothing in this issue will.
- The missing "CREATIVE VENUS" wordmark on every `typography-trick` slide is
  `unsupported-custgeom-picturefill-wordmark-not-drawn`, not this issue; it is visible in
  evidence-1 and evidence-2 and should be ignored when reading them.

*Not confirmed*

- `a:fillToRect` / `a:tileRect` are parsed nowhere; `gradient_paint` always centres a
  path gradient on the shape and uses half the diagonal as the radius
  ([`crates/pptx-raster/src/lib.rs:674-685`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L674-L685)). Every gradient in this cluster uses
  `l/t/r/b="50000"` (dead centre), so this cannot be observed here and is left as a
  separate, unmeasured gap.
- `a:lin/@scaled` and `rotWithShape` are likewise unparsed; no finding in this cluster
  depends on them.

Related issues found in the same run: `fill-alpha-modifier-ignored`, `unsupported-custgeom-picturefill-wordmark-not-drawn`

Files most likely involved: `crates/pptx-render/src/layout.rs`, `crates/pptx-parse/src/drawing.rs`, `crates/pptx-render/src/display_list.rs`, `crates/pptx-raster/src/lib.rs`, `crates/ooxml-drawingml/src/chart/parse.rs`, `crates/ooxml-drawingml/src/chart/model.rs`, `crates/pptx-render/src/chart.rs`, `packages/pptx/src/render/canvas.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/fill-nonsolid-fill-types-not-resolved/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
