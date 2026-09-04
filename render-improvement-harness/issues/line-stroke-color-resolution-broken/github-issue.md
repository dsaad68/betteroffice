# pptx: Connector/line stroke color not resolved from style or gradFill

**Describe the bug**

Whole diagrams lose their lines. On `cisco-cloud-security` slide 4 the 93 connectors that wire the
"sharing network" tree together are all absent, leaving a scatter of unconnected node circles
(evidence-1.png); slide 19 is the same deck asset and loses the same 93 (evidence-4.png). On slide 9
the seven spokes of the risk radar chart are gone, so only the category dots survive and the chart
has no structure left (evidence-2.png). A quieter form of the same gap: the "SSN" badge on slide 16
*is* stroked, but at a 1px hairline instead of the 2pt border the shape's style asks for
(evidence-3.png).

Three related defects sit behind these: a stroke defined only by `p:style/a:lnRef`, a stroke whose
`a:ln` carries a `gradFill`, and a stroke whose width lives in the theme's line-style matrix.

Seen on 4 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/04** the node tree: every `lnRef`-only connector line present in the reference, none in the candidate

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-stroke-color-resolution-broken/evidence-1.png)

**2. cisco-cloud-security/09** the radar chart: seven `<a:ln><a:gradFill>` spokes in the reference, only the endpoint dots in the candidate

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-stroke-color-resolution-broken/evidence-2.png)

**3. cisco-cloud-security/16** the "SSN" badge at 8x: the `FFC000` border is drawn, but 1px wide against the reference's ~2.7px (`lnRef idx="2"` -> `lnStyleLst` `w="25400"`)

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-stroke-color-resolution-broken/evidence-3.png)

**4. cisco-cloud-security/19** three panels - reference, candidate, and the candidate with `p:cxnSp` textually renamed to `p:sp`: parsing the connectors is not enough, the lines stay invisible because `lnRef` supplies their only colour

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-stroke-color-resolution-broken/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 4, 9, 16, 19

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(3, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

### A. `p:style/a:lnRef` is never parsed, so a stroke defined only by the style matrix has no colour (2 findings)

`parse_shape` ([`crates/pptx-parse/src/drawing.rs:138-157`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L138-L157)) reads `spPr` only and never looks at the
sibling `p:style`; `Shape` ([`crates/pptx-parse/src/model.rs:198-207`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L198-L207)) has no field for one. As the
`theme-color-scheme-color-resolution-broken` report established for `fontRef`, none of
`lnRef`/`fillRef`/`fontRef`/`effectRef` appear anywhere in the pptx crates -
`grep -rn --include='*.rs' 'lnRef\|fillRef\|effectRef' crates/` hits only `docx-parse` and an
`xlsx-parse` writer. The half of the matrix those references point into is missing too: `Theme`
([`crates/ooxml-drawingml/src/theme.rs:134-138`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L134-L138)) carries `name`, `color_scheme` and `font_scheme` and
no `format_scheme`, and `parse_theme` ([`crates/pptx-parse/src/theme.rs:5-12`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/theme.rs#L5-L12)) reads `clrScheme` and
`fontScheme` only, so `a:fmtScheme/a:lnStyleLst` is never read.

`parse_outline` ([`crates/pptx-parse/src/drawing.rs:624-643`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L624-L643)) therefore returns
`ShapeOutline { color: None, .. }` for these shapes, and `stroke`
([`crates/pptx-render/src/layout.rs:1930-1944`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1930-L1944)) drops them on its first line:

```rust
fn stroke(outline: &ShapeOutline, theme: &Theme) -> Option<Stroke> {
    let color = resolve_color_value_to_hex_with_theme(outline.color.as_ref(), Some(theme))?;
```

`None` colour, `None` stroke, nothing drawn - at all three call sites
([`crates/pptx-render/src/layout.rs:389-393`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L389-L393), `:528-531`, `:545-548`).

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
`parse_shape_children` ([`crates/pptx-parse/src/drawing.rs:101-135`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L101-L135)) - the subject of
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

`parse_outline` ([`crates/pptx-parse/src/drawing.rs:632`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L632)) reads `line.child("solidFill")` and nothing
else, and there is nowhere to put a gradient anyway: `ShapeOutline`
([`crates/ooxml-drawingml/src/shape.rs:43-57`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/shape.rs#L43-L57)) has `color: Option<ColorValue>` and no gradient field,
unlike `ShapeFill` (`:9-14`), which does. The same narrowing repeats twice more downstream - `Stroke`
in the display list ([`crates/pptx-render/src/display_list.rs:54-59`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L54-L59)) is `{ color: String, width: f32,
dashed: bool }` while `Paint` (`:24-34`) has a `Gradient` variant, and `stroke_paint` in the
rasterizer ([`crates/pptx-raster/src/lib.rs:697-717`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L697-L717)) only ever calls `paint.set_color(...)`. The web
contract narrows identically ([`packages/pptx/src/types.ts:214-218`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L214-L218)).

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
`width: None` and `stroke` ([`crates/pptx-render/src/layout.rs:1934-1938`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1934-L1938)) falls back to
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
  the pptx path - `ThemeColorScheme::get` ([`crates/ooxml-drawingml/src/theme.rs:59-74`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L59-L74)) has no arm
  for it and `default_theme_color` ([`crates/ooxml-drawingml/src/color.rs:183-199`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L183-L199)) sends it to
  `_ => "000000"`. Every `lnStyleLst` entry in this deck's theme fills with `phClr`, so the
  substitution is on the critical path for defect C, and for defect A whenever the deck's line style
  applies a `shade`/`satMod` the `lnRef`'s own colour does not carry.
- **`project17` has 5 `p:cxnSp` with neither an `a:ln` nor a `p:style`.** What PowerPoint draws for
  those is not established here; they are outside this cluster's findings.

**Suggested fix**

### A. Parse the line-style matrix and apply `a:lnRef`

1. [`crates/ooxml-drawingml/src/theme.rs:134`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L134) - add `format_scheme: ThemeFormatScheme` to `Theme`,
   `#[serde(default)]` so serialized decks keep loading. Only the line list is needed now:

   ```rust
   pub struct ThemeFormatScheme { pub line_styles: Vec<ShapeOutline> }
   ```

   `ShapeOutline` already models `w`/`cap`/`prstDash`/`join`, and the `a:ln` inside `lnStyleLst` is
   the same element `parse_outline` reads, so the type fits without inventing a new one. Its `color`
   holds the `phClr` placeholder verbatim.

2. [`crates/pptx-parse/src/theme.rs:5`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/theme.rs#L5) - parse `a:fmtScheme/a:lnStyleLst` into that vector, reusing
   `parse_outline`'s body (move it to a `pub(crate) fn parse_line(&XmlElement) -> ShapeOutline` in
   `drawing.rs` and have both callers use it).

3. [`crates/pptx-parse/src/model.rs:198`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L198) - add `style: Option<ShapeStyle>` to `Shape`
   (`#[serde(default, skip_serializing_if = "Option::is_none")]`), with

   ```rust
   pub struct ShapeStyle {
       pub line_ref: Option<StyleRef>,      // idx + colour
       pub font_ref_color: Option<ColorValue>,
       pub font_ref: Option<String>,
   }
   pub struct StyleRef { pub index: u32, pub color: Option<ColorValue> }
   ```

   and fill it in `parse_shape` ([`crates/pptx-parse/src/drawing.rs:138`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L138)) from the sibling `p:style`.
   `parse_color_container` (`:654`) already reads the colour, `a:shade`/`a:satMod` included.

   Keep the reference *unresolved* in the model. Do **not** bake a synthesized outline into
   `Shape.outline`: `patch_shape` ([`crates/pptx-parse/src/write.rs:866`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L866)) and the editor's stroke path
   ([`crates/pptx-edit/src/deck.rs:489-508`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L489-L508)) round-trip `ShapeOutline` verbatim, so a synthesized one
   would materialize an explicit `<a:ln>` into a file that never had one the first time anything
   touches that shape's stroke.

4. [`crates/pptx-render/src/layout.rs:1930`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1930) - resolve at render time. `stroke` grows the shape's
   style and merges: start from `lnStyleLst[idx - 1]` (`idx = 0` means "no line", return `None`),
   substitute the `lnRef`'s own colour for every `phClr` in it, then overlay whatever the explicit
   `a:ln` states. That is what makes 16/6 (colour from `a:ln`, width from the matrix) and 04/2
   (everything from the matrix) fall out of one code path.

5. [`crates/ooxml-drawingml/src/color.rs:61`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L61) - `phClr` needs a substitution hook, since
   `ThemeColorScheme::get` ([`crates/ooxml-drawingml/src/theme.rs:59`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L59)) has no arm for it and
   `default_theme_color` (`:183`) would turn it black. Cheapest form: resolve the `phClr` slot into
   the reference's colour *before* calling the resolver, keeping the matrix entry's own
   `shade`/`satMod` modifiers.

   [`crates/docx-parse/src/shape.rs:698-712`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/shape.rs#L698-L712) is the prior art, and also the cheap fallback if the
   `fmtScheme` work has to be deferred: it ignores `lnStyleLst` entirely and synthesizes
   `width: 9525` plus the `lnRef`'s colour. That alone fixes 04/2 and 19/2, but not 16/6.

### B. Let a stroke carry a gradient

6. [`crates/ooxml-drawingml/src/shape.rs:43`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/shape.rs#L43) - add `gradient: Option<GradientFill>` to
   `ShapeOutline`, mirroring `ShapeFill` (`:9-14`), and read `a:gradFill` in `parse_outline`
   ([`crates/pptx-parse/src/drawing.rs:632`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L632)) via the existing `parse_gradient_fill` (`:585`).

7. [`crates/pptx-render/src/display_list.rs:54`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L54) - add
   `#[serde(default, skip_serializing_if = "Option::is_none")] pub paint: Option<Paint>` to
   `Stroke`, keeping `color` as the flattened first-stop fallback. Additive and optional, so
   `CONTRACT_VERSION` stays 1 and any consumer that only reads `color` still draws a plausible line.
   Mirror the field in [`packages/pptx/src/types.ts:214`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L214).

8. [`crates/pptx-raster/src/lib.rs:697`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L697) - `stroke_paint` takes the shape box and calls the existing
   `gradient_paint` (`:627`) when `stroke.paint` is a gradient. `paint_shape` (`:364`) and
   `paint_image` (`:390`) already have `x, y, w, h` in scope; only `stroke_path` (`:482`) needs them
   threaded through.

```rust
// crates/pptx-render/src/layout.rs
fn stroke(outline: Option<&ShapeOutline>, style: Option<&ShapeStyle>, theme: &Theme) -> Option<Stroke> {
    let reference = style.and_then(|style| style.line_ref.as_ref());
    let base = match reference {
        Some(reference) if reference.index == 0 => return None,
        Some(reference) => theme
            .format_scheme
            .line_styles
            .get(reference.index as usize - 1)
            .map(|line| substitute_placeholder(line, reference.color.as_ref())),
        None => None,
    };
    let merged = merge(base.as_ref(), outline)?; // explicit a:ln wins property by property
    let paint = merged
        .gradient
        .as_ref()
        .and_then(|gradient| gradient_paint_from(gradient, theme));
    let color = resolve_color_value_to_hex_with_theme(merged.color.as_ref(), Some(theme))
        .or_else(|| first_stop_hex(paint.as_ref()))?;
    Some(Stroke { color, paint, width: /* unchanged */, dashed: /* unchanged */ })
}
```

```rust
// crates/pptx-parse/src/drawing.rs, parse_shape
style: element.child("style").map(|style| ShapeStyle {
    line_ref: style.child("lnRef").map(|reference| StyleRef {
        index: reference.attribute("idx").and_then(|v| v.parse().ok()).unwrap_or(0),
        color: parse_color_container(reference),
    }),
    font_ref_color: style.child("fontRef").and_then(parse_color_container),
    font_ref: style.child("fontRef").and_then(|v| v.attribute("idx")).map(str::to_owned),
}),
```

Risks and tests to add:

- **The width change reaches 83 shapes the findings never mention** - every `a:ln` with a visible
  fill but no `w` under a `lnRef idx >= 1`: 66 on `cisco-cloud-security`, 16 on `project20`, 1 on
  `ocp-psp-plan`. They all get thicker. That is correct per spec and matches LibreOffice on 16/6,
  but it is the one part of this change that can move slides nobody looked at. `project20` is the
  canary because it is otherwise close to the reference.
- **The colour change reaches nothing else in this corpus.** Zero of 2546 `p:sp` need `lnRef` for
  their stroke colour (they all carry an explicit `a:ln`, usually `<a:ln><a:noFill/></a:ln>`), so
  step 4's blast radius outside connectors is the width only. The `idx="0"` -> no-line rule must be
  honoured or that flips: `lnRef idx="0"` is common on shapes whose `a:ln` is `noFill`.
- **`phClr` half-done is worse than not done.** If the matrix entry is applied but the placeholder
  is not substituted, `default_theme_color` ([`crates/ooxml-drawingml/src/color.rs:183`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L183)) paints every
  themed line `000000`. Add a regression test that a `phClr` line style resolves to the `lnRef`'s
  colour and never to black.
- **Round trip.** Keeping the reference in `Shape.style` and resolving in `pptx-render` is what makes
  this safe. `cargo test -p pptx-edit --test write_fidelity` must stay green, and a case where a
  stroke edit lands on a shape whose only outline came from `lnRef` is worth adding - the editor
  ([`crates/pptx-edit/src/deck.rs:489`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L489)) starts from `outlineJson`, which will still be `None` there.
- **Gradient direction on a degenerate box.** The radar spokes are `ext cx="0" cy="830086"`, a
  zero-width box, with `<a:lin ang="5400000"/>`. `gradient_paint`
  ([`crates/pptx-raster/src/lib.rs:627`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L627)) sizes its shader off `w.hypot(h)`, which survives a zero
  side, but the stroke is drawn *outside* that box by half the line width - check the ends do not
  land on `SpreadMode::Pad` fringe.
- **The canvas backend must match.** `packages/pptx` renders the same display list; a `Stroke.paint`
  the web backend ignores means raster and browser disagree on seven visible lines. The
  fallback `color` keeps that a colour difference rather than a missing line.
- **Tests to add** (none exist for any of the three mechanisms): a `pptx-parse` case that a `p:style`
  with `lnRef` and an `a:ln` with `gradFill` both reach the model; a `pptx-render` case that a shape
  with no `a:ln` and `lnRef idx="1"` gets the theme width and the reference colour, that an explicit
  `a:ln` colour beats the matrix while the matrix width still applies, and that `idx="0"` strokes
  nothing; a `pptx-raster` golden with a gradient-stroked line.

**How to verify**

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
([`crates/pptx-render/src/layout.rs:2008-2500`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2008-L2500)) asserts on geometry, glyphs, autofit and charts and
never on a `Stroke`; the raster golden ([`crates/pptx-raster/tests/golden.rs:80-86`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/tests/golden.rs#L80-L86)) builds a
`SurfaceDisplayList` by hand and has no stroked gradient. All three need new tests.

**Additional context**

none.

Related issues found in the same run: `geometry-custom-collapses-to-bbox`, `line-zero-extent-skipped`, `theme-color-scheme-color-resolution-broken`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/theme.rs`, `crates/ooxml-drawingml/src/theme.rs`, `crates/ooxml-drawingml/src/shape.rs`, `crates/ooxml-drawingml/src/color.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/display_list.rs`, `crates/pptx-raster/src/lib.rs`, `packages/pptx/src/types.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/line-stroke-color-resolution-broken/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
