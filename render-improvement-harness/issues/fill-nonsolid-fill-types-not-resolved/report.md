---
id: fill-nonsolid-fill-types-not-resolved
title: Sort gradient stops and resolve pattFill instead of collapsing both to a flat fill
category: fill
impact: medium
effort: medium
confidence: high
status: open
occurrences: 6
decks: [cisco-cloud-security, minimal-chart, typography-trick]
findings: [cisco-cloud-security/03/2, cisco-cloud-security/07/5, minimal-chart/01/3, typography-trick/01/2, typography-trick/02/2, typography-trick/03/2]
files: [crates/pptx-render/src/layout.rs, crates/pptx-parse/src/drawing.rs, crates/pptx-render/src/display_list.rs, crates/pptx-raster/src/lib.rs, crates/ooxml-drawingml/src/chart/parse.rs, crates/ooxml-drawingml/src/chart/model.rs, crates/pptx-render/src/chart.rs, packages/pptx/src/render/canvas.ts]
---

## Symptom

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

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | typography-trick/03 | reference has a blue radial glow at the centre (`accent1` lumMod 50%, the `pos="0"` stop); candidate is flat `#222A35`, the `pos="100000"` stop |
| 2 | typography-trick/02 | same failure in greyscale: reference centre `#404040`, corners `#262626`; candidate is `#262626` everywhere |
| 3 | cisco-cloud-security/07 | reference's "Analyze & Control" band carries the `pct30` cross-hatch; candidate's band is flat white |
| 4 | minimal-chart/01 | reference chart space is filled with the teal `smGrid` pattern; candidate's chart space is unfilled white |

Sampled pixels (centre vs. top-left corner, `bo-img` and `lo-img` at 1280x720):

| slide | BO centre | BO corner | LO centre | LO corner |
|---|---|---|---|---|
| typography-trick/01 | (214,220,229) | (214,220,229) | (255,255,255) | (214,220,229) |
| typography-trick/02 | (38,38,38) | (38,38,38) | (64,64,64) | (38,38,38) |
| typography-trick/03 | (34,42,53) | (34,42,53) | (32,56,100) | (34,42,53) |

## Root cause (confirmed)

### A. Gradient stops are emitted in document order and never sorted

`parse_gradient_fill` collects `a:gs` children in document order
(`crates/pptx-parse/src/drawing.rs:594`) and `paint` maps them straight into the display
list without sorting (`crates/pptx-render/src/layout.rs:1908`). All three
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

`gradient_paint` (`crates/pptx-raster/src/lib.rs:627`) hands those stops to tiny-skia
unchanged. `tiny_skia::Gradient::new` brackets the list with dummy stops and then *pins
positions monotonically*
(`~/.cargo/registry/src/index.crates.io-*/tiny-skia-0.12.0/src/shaders/gradient.rs:78-93`,
`stops[i].position.get().bound(prev, 1.0)`). For `[1.0 -> #262626, 0.0 -> #404040]` that
yields positions `[0, 1, 1, 1]` with colours `[#262626, #262626, #404040, #404040]`: the
whole 0..1 range is `#262626`. That is exactly the flat colour observed, for all three
slides.

The web backend does *not* have this bug — `packages/pptx/src/render/canvas.ts:197` uses
`addColorStop`, and the canvas spec sorts stops by offset — so this is also a
raster/canvas parity divergence, not only a raster defect.

Scope beyond this cluster: 15 out-of-order `a:gsLst` elements across 9 parts in the
harness corpus (`cisco-cloud-security/07`, `/09`, `rollout-plan` layouts 05/06/09,
`triangles-corporate/01`, all three `typography-trick` slides), against 690 already-sorted
ones. Fixing the ordering fixes those too.

### B. `a:pattFill` is not in the parser at all

`parse_fill` (`crates/pptx-parse/src/drawing.rs:565`) recognises only `noFill`,
`solidFill`, `gradFill` and `blipFill`. There is no `pattFill` branch anywhere in the pptx
crates — the only occurrences of the string are `docx-parse` and the writer's
`FILL_ELEMENTS` round-trip list at `crates/pptx-parse/src/write.rs:1000`. A pattern-filled
shape therefore parses to `fill: None` and, at `crates/pptx-render/src/layout.rs:384-388`,
is drawn unfilled. There is no `Paint` variant to carry a pattern either
(`crates/pptx-render/src/display_list.rs:24-34`: `Solid` and `Gradient` only).

Corpus counts: `ltUpDiag` x3 and `pct30` x1 in `cisco-cloud-security/07`, `smGrid` x1 in
`minimal-chart`'s chart part.

### C. The chart space has no fill of any kind (minimal-chart/01/3 needs this too)

`minimal-chart`'s pattern sits on `c:chartSpace/c:spPr`. `parse_chart_space`
(`crates/ooxml-drawingml/src/chart/parse.rs:72`) never reads `c:spPr` for the chart space
or the plot area, and `ChartSpace` (`crates/ooxml-drawingml/src/chart/model.rs:5-24`) has
no fill field — the parser only ever looks for `solidFill` under a series, data point,
marker or run. The chart sink can only emit solid rectangles anyway
(`crates/pptx-render/src/chart.rs:111-118`, `PlotOp::Rect { fill }` -> `Paint::Solid`). So
even a plain `solidFill` chart-space background is dropped today; fixing B alone will not
make evidence-4 correct.

## Not this cluster

- `cisco-cloud-security/03/2` — an `a:lin` gradient whose two stops are the same resolved
  RGB and differ only in `a:alpha` (42000 -> 0). `resolve_color_value_to_hex_with_theme`
  (`crates/ooxml-drawingml/src/color.rs:61-87`) returns opaque `#RRGGBB` and drops alpha,
  so the "gradient" is a flat opaque wash. The sibling resolver
  `resolve_color_value_to_rgba_hex` (`crates/ooxml-drawingml/src/color.rs:91`) already
  exists and `pptx-raster`'s `parse_hex_color` already accepts 8-digit hex
  (`crates/pptx-raster/src/lib.rs:780-796`). It shares its root cause with
  `fill-alpha-modifier-ignored`, whose fix resolves it; nothing in this issue will.
- The missing "CREATIVE VENUS" wordmark on every `typography-trick` slide is
  `unsupported-custgeom-picturefill-wordmark-not-drawn`, not this issue; it is visible in
  evidence-1 and evidence-2 and should be ignored when reading them.

## Not confirmed

- `a:fillToRect` / `a:tileRect` are parsed nowhere; `gradient_paint` always centres a
  path gradient on the shape and uses half the diagonal as the radius
  (`crates/pptx-raster/src/lib.rs:674-685`). Every gradient in this cluster uses
  `l/t/r/b="50000"` (dead centre), so this cannot be observed here and is left as a
  separate, unmeasured gap.
- `a:lin/@scaled` and `rotWithShape` are likewise unparsed; no finding in this cluster
  depends on them.

## Verification

1. `.venv/bin/python render-improvement-harness/scripts/pipeline.py` (or `render_bo.py` +
   `diff.py`) for `typography-trick`. After the stop-order fix, all three slides' `bo-img`
   centre pixel must match the `pos="0"` stop, not the `pos="100000"` one: `(255,255,255)`
   for 01, `(64,64,64)` for 02, `(32,56,100)` for 03, with a smooth falloff to the corner
   values already matching. Slide 02's `fine_pct` should fall from 3.54 toward ~2, slide
   03's from 6.52 toward ~4 (the wordmark, a different cluster, keeps the rest).
2. Re-render `cisco-cloud-security/07` and `minimal-chart/01` after the pattern work; the
   band and the chart space must stop being white.
3. Existing coverage to extend: the display-list gradient assertion at
   `crates/pptx-render/src/lib.rs:489-495`, the raster golden suite under
   `crates/pptx-raster/tests/golden.rs`, and `packages/pptx/src/render/canvas.test.ts` for
   backend parity. None of them currently feeds a descending `gsLst`.
