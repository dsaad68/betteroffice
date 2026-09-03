---
id: line-stroke-color-resolution-broken
title: Resolve a stroke from p:style/a:lnRef and from a gradFill on a:ln
category: connector
impact: medium
effort: medium
confidence: high
status: open
occurrences: 4
decks: [cisco-cloud-security]
findings: [cisco-cloud-security/04/2, cisco-cloud-security/09/4, cisco-cloud-security/16/6, cisco-cloud-security/19/2]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/pptx-parse/src/theme.rs, crates/ooxml-drawingml/src/theme.rs, crates/ooxml-drawingml/src/shape.rs, crates/ooxml-drawingml/src/color.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/display_list.rs, crates/pptx-raster/src/lib.rs, packages/pptx/src/types.ts]
---

## Symptom

Whole diagrams lose their lines. On `cisco-cloud-security` slide 4 the 93 connectors that wire the
"sharing network" tree together are all absent, leaving a scatter of unconnected node circles
(evidence-1.png); slide 19 is the same deck asset and loses the same 93 (evidence-4.png). On slide 9
the seven spokes of the risk radar chart are gone, so only the category dots survive and the chart
has no structure left (evidence-2.png). A quieter form of the same gap: the "SSN" badge on slide 16
*is* stroked, but at a 1px hairline instead of the 2pt border the shape's style asks for
(evidence-3.png).

Three related defects sit behind these: a stroke defined only by `p:style/a:lnRef`, a stroke whose
`a:ln` carries a `gradFill`, and a stroke whose width lives in the theme's line-style matrix.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/04 | the node tree: every `lnRef`-only connector line present in the reference, none in the candidate |
| 2 | cisco-cloud-security/09 | the radar chart: seven `<a:ln><a:gradFill>` spokes in the reference, only the endpoint dots in the candidate |
| 3 | cisco-cloud-security/16 | the "SSN" badge at 8x: the `FFC000` border is drawn, but 1px wide against the reference's ~2.7px (`lnRef idx="2"` -> `lnStyleLst` `w="25400"`) |
| 4 | cisco-cloud-security/19 | three panels - reference, candidate, and the candidate with `p:cxnSp` textually renamed to `p:sp`: parsing the connectors is not enough, the lines stay invisible because `lnRef` supplies their only colour |

## Root cause (confirmed)

### A. `p:style/a:lnRef` is never parsed, so a stroke defined only by the style matrix has no colour (2 findings)

`parse_shape` (`crates/pptx-parse/src/drawing.rs:138-157`) reads `spPr` only and never looks at the
sibling `p:style`; `Shape` (`crates/pptx-parse/src/model.rs:198-207`) has no field for one. As the
`theme-color-scheme-color-resolution-broken` report established for `fontRef`, none of
`lnRef`/`fillRef`/`fontRef`/`effectRef` appear anywhere in the pptx crates -
`grep -rn --include='*.rs' 'lnRef\|fillRef\|effectRef' crates/` hits only `docx-parse` and an
`xlsx-parse` writer. The half of the matrix those references point into is missing too: `Theme`
(`crates/ooxml-drawingml/src/theme.rs:134-138`) carries `name`, `color_scheme` and `font_scheme` and
no `format_scheme`, and `parse_theme` (`crates/pptx-parse/src/theme.rs:5-12`) reads `clrScheme` and
`fontScheme` only, so `a:fmtScheme/a:lnStyleLst` is never read.

`parse_outline` (`crates/pptx-parse/src/drawing.rs:624-643`) therefore returns
`ShapeOutline { color: None, .. }` for these shapes, and `stroke`
(`crates/pptx-render/src/layout.rs:1930-1944`) drops them on its first line:

```rust
fn stroke(outline: &ShapeOutline, theme: &Theme) -> Option<Stroke> {
    let color = resolve_color_value_to_hex_with_theme(outline.color.as_ref(), Some(theme))?;
```

`None` colour, `None` stroke, nothing drawn - at all three call sites
(`crates/pptx-render/src/layout.rs:389-393`, `:528-531`, `:545-548`).

Confirmed on `cisco-cloud-security` slide 4: 92 of the 93 `p:cxnSp` have no `<a:ln>` in `spPr` at
all, and the 93rd has `<a:ln><a:tailEnd type="arrow" w="med" len="sm"/></a:ln>` with no fill child,
so all 93 depend on the style:

```xml
<p:spPr><a:xfrm flipV="1">...</a:xfrm><a:prstGeom prst="line"><a:avLst/></a:prstGeom></p:spPr>
<p:style><a:lnRef idx="1"><a:schemeClr val="accent1"/></a:lnRef>...</p:style>
```

with `theme1.xml` `accent1 = 214794` and `lnStyleLst[0]` = `<a:ln w="9525" cap="flat" ...>` filled
with `phClr` shaded 95%.

**These connectors also never reach the renderer**, because `p:cxnSp` has no branch in
`parse_shape_children` (`crates/pptx-parse/src/drawing.rs:101-135`) - the subject of
`line-zero-extent-skipped`. The two gaps stack, and evidence-4.png separates them. Taking the deck
at HEAD (`82b04ef`), renaming `p:cxnSp`/`p:nvCxnSpPr`/`p:cNvCxnSpPr` to their `p:sp` equivalents on
slides 4, 9, 16 and 19 and changing nothing else, then reading the display list through the Python
binding:

```
slide 4:  connectors reaching display list = 93, with a stroke = 0
slide 9:  connectors reaching display list = 11, with a stroke = 4
slide 16: connectors reaching display list =  4, with a stroke = 4
slide 19: connectors reaching display list = 93, with a stroke = 0
```

Slides 4 and 19 parse all 93 connectors and stroke none of them. Fixing `line-zero-extent-skipped`
alone will not put a single line back on those two slides.

### B. A `gradFill` on `a:ln` is dropped, end to end (1 finding)

`parse_outline` (`crates/pptx-parse/src/drawing.rs:632`) reads `line.child("solidFill")` and nothing
else, and there is nowhere to put a gradient anyway: `ShapeOutline`
(`crates/ooxml-drawingml/src/shape.rs:43-57`) has `color: Option<ColorValue>` and no gradient field,
unlike `ShapeFill` (`:9-14`), which does. The same narrowing repeats twice more downstream - `Stroke`
in the display list (`crates/pptx-render/src/display_list.rs:54-59`) is `{ color: String, width: f32,
dashed: bool }` while `Paint` (`:24-34`) has a `Gradient` variant, and `stroke_paint` in the
rasterizer (`crates/pptx-raster/src/lib.rs:697-717`) only ever calls `paint.set_color(...)`. The web
contract narrows identically (`packages/pptx/src/types.ts:214-218`).

The seven radar spokes on slide 9 are exactly this shape:

```xml
<a:ln w="19050"><a:gradFill><a:gsLst>
  <a:gs pos="0"><a:srgbClr val="C00000"/></a:gs>
  <a:gs pos="28000"><a:srgbClr val="C00000"/></a:gs>
  <a:gs pos="30000"><a:srgbClr val="C2C2C2"/></a:gs>
  <a:gs pos="100000"><a:srgbClr val="C2C2C2"/></a:gs>
</a:gsLst><a:lin ang="5400000" scaled="1"/></a:gradFill></a:ln>
```

`w="19050"` survives; the colour does not, so `stroke` bails on the same `?`. The
rename-to-`p:sp` run above shows the split cleanly: of slide 9's 11 connectors, the 4 with
`<a:ln><a:solidFill><a:schemeClr val="bg1"/></a:solidFill></a:ln>` get
`{'color': '#FFFFFF', 'width': 1.0}` and the 7 gradient ones get nothing.

### C. The line-style matrix also supplies width and dash, not just colour (1 finding)

`Rectangle 19` on slide 16 (the "SSN" badge) does carry an explicit `a:ln`, but only a fill:

```xml
<a:ln><a:solidFill><a:srgbClr val="FFC000"/></a:solidFill></a:ln>
<p:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef>
```

Per ECMA-376 an explicit `a:ln` overrides only the properties it states; `w`, `cap`, `cmpd` and
`prstDash` still come from `lnStyleLst[idx-1]`, here `<a:ln w="25400" cap="flat" cmpd="sng"
algn="ctr">` = 2pt = 2.67px at 96 dpi. Because nothing parses the matrix, `parse_outline` leaves
`width: None` and `stroke` (`crates/pptx-render/src/layout.rs:1934-1938`) falls back to
`unwrap_or(1.0)`.

Measured at HEAD, a vertical pixel scan through the badge's top edge at x = 473:

```
reference: ... (251,202,75) (253,198,45) (255,192,0) (255,192,0) (255,238,187) (255,255,255) ...
candidate: ... (251,202,75) (251,202,75) (255,192,0) (255,255,255) (255,255,255) (255,255,255) ...
```

~2.5px of `FFC000` against 1px, and the display list confirms
`Rectangle 19 {'color': '#FFC000', 'width': 1.0}`.

### Scale in the corpus

Counted over every `ppt/slides/slideN.xml` in `render-improvement-harness/decks`:

| | `p:sp` | `p:cxnSp` |
|---|---|---|
| total | 2546 | 309 |
| needs `lnRef` for its stroke colour (no fill child on `a:ln`, `lnRef idx` >= 1) | 0 | 192 |
| `gradFill` on `a:ln` | 0 | 7 |

Plus 83 shapes across `cisco-cloud-security` (66), `project20` (16) and `ocp-psp-plan` (1) that have
a visible `a:ln` with no `w` under a `lnRef idx >= 1` - defect C, currently drawn 1px.

So defects A and B are, in this corpus, purely a connector problem; defect C is the one that reaches
ordinary autoshapes.

### Not confirmed

- **`cisco-cloud-security/16/6`'s claim that the "SSN" tag "loses its orange outline" is wrong.**
  The outline is drawn, in the right colour; only its width is wrong (evidence-3.png). The finding's
  severity is right but its description should read "outline drawn 1px instead of the style's 2pt".
- **Findings 04/2 and 19/2 say "the candidate is evidently not resolving this style-matrix
  reference".** True, but incomplete: the connectors are also dropped at parse time before any
  resolution could happen. Neither fix alone puts a line on those slides.
- **Whether `phClr` substitution needs to be built or merely wired.** `phClr` is handled nowhere in
  the pptx path - `ThemeColorScheme::get` (`crates/ooxml-drawingml/src/theme.rs:59-74`) has no arm
  for it and `default_theme_color` (`crates/ooxml-drawingml/src/color.rs:183-199`) sends it to
  `_ => "000000"`. Every `lnStyleLst` entry in this deck's theme fills with `phClr`, so the
  substitution is on the critical path for defect C, and for defect A whenever the deck's line style
  applies a `shade`/`satMod` the `lnRef`'s own colour does not carry.
- **`project17` has 5 `p:cxnSp` with neither an `a:ln` nor a `p:style`.** What PowerPoint draws for
  those is not established here; they are outside this cluster's findings.

## Verification

Re-render `cisco-cloud-security` with
`.venv/bin/python render-improvement-harness/scripts/render_bo.py cisco-cloud-security` then
`diff.py cisco-cloud-security`.

- Slides 04 (9.66%) and 19 (8.72%): all 93 connectors per slide must appear as ~1px `#1F438D`
  lines. This lands only once `line-zero-extent-skipped` has landed as well; the residual diff on
  both slides stays dominated by `geometry-custom-collapses-to-bbox` (131 `custGeom` shapes each).
- Slide 09 (10.86%): the seven radar spokes must appear, each shading `C00000` -> `C2C2C2` (or
  `57B74E`/`FFC000` -> `C2C2C2`) along its length at `w="19050"` = 1.5pt. The finding calls this the
  slide's single largest contributor, hitting hot cells `r3c4` and `r3c1`.
- Slide 16 (15.44%): the "SSN" badge border thickens from 1px to ~2.67px. A small win; the slide's
  diff is carried by other clusters.
- Regression watch: the 83 shapes in defect C's count get thicker outlines. `project20`'s 16 are the
  ones to eyeball, since that deck is otherwise close to the reference.

No test in the tree covers any of the three mechanisms. `parse_outline` has no unit test in
`crates/pptx-parse/src/drawing.rs`; the `pptx-render` test module
(`crates/pptx-render/src/layout.rs:2008-2500`) asserts on geometry, glyphs, autofit and charts and
never on a `Stroke`; the raster golden (`crates/pptx-raster/tests/golden.rs:80-86`) builds a
`SurfaceDisplayList` by hand and has no stroked gradient. All three need new tests.
