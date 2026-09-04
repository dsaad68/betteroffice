# pptx: Parse p:style/a:fontRef and the clrMap/clrMapOvr colour map

**Describe the bug**

Text that should be white comes out in the theme's dark text colour. On `cisco-cloud-security`
slide 6 all three callout headings render `#676767` instead of white, and "Intelligent Protection"
sits on an `accent3` fill that is *also* `#676767`, so the heading is completely invisible
(evidence-1.png). The same happens to every column header and body block on `project20` slide 3,
while the one run on that slide that carries its own explicit `schemeClr` still renders white
(evidence-2.png). A second, rarer form hits whole slides: `rollout-plan` slide 1 loses its purple
background *and* its white title together (evidence-3.png), and `ocp-psp-plan` slide 14's title
comes out plain black on dark blue (evidence-4.png).

These are two independent defects that produce the same-looking failure: 17 findings from the
first and 3 from the second.

Seen on 20 slides across 4 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/06** three `p:style/a:fontRef` `lt1` headings drawn `#676767`; on the `accent3` box the heading vanishes into its own fill

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/theme-color-scheme-color-resolution-broken/evidence-1.png)

**2. project20/03** the same `fontRef` failure on four column headers and four body blocks, next to a title whose run-level `schemeClr bg1` *does* resolve to white - isolating the failure to `fontRef`, not to scheme colours in general

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/theme-color-scheme-color-resolution-broken/evidence-2.png)

**3. rollout-plan/01** full slide: layout `clrMapOvr` ignored, so `bg2` paints `lt2` grey instead of `dk2` purple and the title's `tx1` paints `dk1` grey instead of `lt1` white

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/theme-color-scheme-color-resolution-broken/evidence-3.png)

**4. ocp-psp-plan/14** the same layout `clrMapOvr` on a section divider; here the title lands on the hardcoded `#000000` fallback rather than on any theme colour

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/theme-color-scheme-color-resolution-broken/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 4, 6, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20, 23
- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 14
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 3, 4
- `rollout-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 9, 11

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

### A. `p:style/a:fontRef` is never parsed (17 findings)

`fontRef` appears nowhere in the pptx crates - `grep -rn "fontRef\|fillRef\|lnRef\|effectRef" crates/`
returns only `crates/docx-parse/src/shape.rs`. `parse_shape`
([`crates/pptx-parse/src/drawing.rs:138-156`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L138-L156)) reads `spPr`, `prstGeom`, fill, outline and `txBody`
and never looks at the sibling `p:style` element; `Shape`
([`crates/pptx-parse/src/model.rs:198-207`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L198-L207)) has no field to put it in.

An autoshape's `p:style/a:fontRef` supplies the default text colour for every run in that shape
that does not set one. With it missing, `resolve_style`
([`crates/pptx-render/src/layout.rs:1042-1051`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1042-L1051)) falls through to the paragraph cascade, whose base
for a non-placeholder shape is the master's `p:otherStyle`
([`crates/pptx-render/src/layout.rs:808-812`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L808-L812), `:1786-1801`) - and every deck in this cluster sets
that to `schemeClr tx1`.

Confirmed on `cisco-cloud-security` slide 6, id 46:

```xml
<a:solidFill><a:schemeClr val="accent3"/></a:solidFill>        <p:style>...<a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style>
<a:r><a:rPr lang="en-US" sz="2200" b="1" dirty="0"/><a:t>Intelligent Protection</a:t></a:r>
```

`theme1.xml` has `dk1 = 676767`, `lt1 = FFFFFF`, `accent3 = 676767`; the master's `otherStyle`
`lvl1pPr/defRPr` is `<a:solidFill><a:schemeClr val="tx1"/></a:solidFill>`. Rendering the deck
through the Python binding at HEAD (`b21db5f`) returns

```
'Intelligent Protection'      -> #676767      (expected #FFFFFF)
'SaaS Visibility'             -> #676767
'Extended \nGranular Control' -> #676767
```

and `project20` slide 3 returns `'Playbook' -> #505050`, `'Conversation Guides' -> #505050`
(theme `dk1 = 505050`) beside `'Workstreams ' -> #FFFFFF`, the one run that carries
`<a:solidFill><a:schemeClr val="bg1"/></a:solidFill>` itself.

Note the master `otherStyle` should not be in this chain at all - for a slide shape the fallback
below `fontRef` is `p:defaultTextStyle` in `presentation.xml`, which is also unparsed. That is a
separate (and here invisible) inaccuracy; it is only because `otherStyle` is consulted that the
failure shows up as theme grey rather than as the `#000000` literal at
[`crates/pptx-render/src/layout.rs:1051`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1051).

### B. `p:clrMap` / `p:clrMapOvr` are never parsed (3 findings)

The colour map is not in the model: `SlideMaster` ([`crates/pptx-parse/src/model.rs:102-110`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L102-L110)),
`SlideLayout` (`:90-99`) and `Slide` (`:78-86`) carry no map field, and neither
`common_slide_data` ([`crates/pptx-parse/src/drawing.rs:41-63`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L41-L63)) nor the master/layout loops in
[`crates/pptx-parse/src/package.rs:75-128`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/package.rs#L75-L128) read `p:clrMap` or `p:clrMapOvr`. The only occurrence in
the tree is on the write path ([`crates/pptx-parse/src/write.rs:1832`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1832)), which emits a hardcoded
`<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>` for synthesized slides.

Instead the standard map is baked in at parse time. `normalize_scheme_color`
([`crates/pptx-parse/src/drawing.rs:695-703`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L695-L703)) rewrites `tx1`->`text1`, `bg1`->`background1`,
`tx2`->`text2`, `bg2`->`background2`, and `ThemeColorScheme::get`
([`crates/ooxml-drawingml/src/theme.rs:59-74`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L59-L74)) then reads `text1` as `dk1` and `background1` as
`lt1`. That is exactly the master's default `clrMap`, and it cannot be overridden.

Confirmed on `rollout-plan` slide 1:

```xml
<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" .../>
<p:clrMapOvr><a:overrideClrMapping bg1="dk1" tx1="lt1" bg2="dk2" tx2="lt2" .../></p:clrMapOvr>
<p:bg><p:bgRef idx="1001"><a:schemeClr val="bg2"/></p:bgRef></p:bg>
<a:solidFill><a:schemeClr val="tx1"/></a:solidFill>
```

with theme `dk1 = 505050`, `lt1 = FFFFFF`, `dk2 = 68217A`, `lt2 = D2D2D2`. Under the override the
background must be `#68217A` and the title `#FFFFFF`; the renderer produces `#D2D2D2` and,
measured from the display list at HEAD, `'Change Management Roll-out Plan ' -> #505050`.
`rollout-plan` slide 9 uses the same layout family and yields `'Appendix' -> #505050` where
`#FFFFFF` is required.

A landmine for whoever fixes this: `ThemeColorScheme::get`
([`crates/ooxml-drawingml/src/theme.rs:59-74`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L59-L74)) has no arm for the raw `tx1`/`bg1`/`tx2`/`bg2` names
(nor for `phClr`), and `default_theme_color` ([`crates/ooxml-drawingml/src/color.rs:183-199`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L183-L199)) sends
anything unrecognised to `_ => "000000"`. Stopping the parse-time normalization without adding a
map-aware lookup turns every mapped colour black.

### Not confirmed

- **`ocp-psp-plan/14/2` is not a pure `clrMapOvr` failure.** The layout does carry
  `<a:overrideClrMapping tx1="lt1" .../>`, but the title renders `#000000`, not the `#505050` that
  the unoverridden map would give. The colour it should have inherited lives in the layout
  placeholder's own `lstStyle` (`<a:defRPr sz="6000"><a:solidFill><a:schemeClr val="tx1"/>`), which
  is dropped by `text-inheritance-layout-lststyle-ignored`, and the master's `p:titleStyle` uses
  `<a:gradFill>` with two `tx1` stops, dropped by `text-run-props-gradfill-not-resolved`. With no
  colour reaching it at all, `resolve_style` lands on the `"#000000"` literal at
  [`crates/pptx-render/src/layout.rs:1051`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1051). Fixing the colour map is necessary but not sufficient
  for this slide. `rollout-plan/09/1` has the same `gradFill` construct in its master `titleStyle`,
  though there a `tx1` colour does reach the run.
- **`project20/03/1`'s claim that an explicit run-level `schemeClr` also fails is wrong.** The
  "Workstreams" title's `<a:schemeClr val="bg1"/>` resolves correctly to `#FFFFFF` (evidence-2.png,
  and measured from the display list). Only the `fontRef`-coloured shapes on that slide fail.
- The style matrix behind `fillRef`/`lnRef`/`effectRef` is a *separate* problem: `Theme`
  ([`crates/ooxml-drawingml/src/theme.rs:126-131`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L126-L131)) has no `formatScheme`, so those three references
  cannot be resolved at all. `fontRef` does not need it - its colour is a direct child of the
  element, and `idx="minor"`/`"major"` maps onto the font scheme that is already parsed.

**Suggested fix**

### A. Carry `p:style/a:fontRef` as the shape's default text colour

1. [`crates/pptx-parse/src/model.rs:198`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L198) - add an optional style to `Shape`:

   ```rust
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub style: Option<ShapeStyle>,
   ```

   with `pub struct ShapeStyle { pub font_ref_color: Option<ColorValue>, pub font_ref: Option<String> }`
   (`font_ref` being `idx`, i.e. `minor`/`major`, so `+mn-lt`/`+mj-lt` can be fed to the existing
   `resolve_theme_font_ref`). `serde(default)` keeps packages serialized before this change loading.

2. [`crates/pptx-parse/src/drawing.rs:138`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L138) - in `parse_shape`, read the sibling element. The colour
   is a direct child of `a:fontRef`, so `parse_color_container`
   ([`crates/pptx-parse/src/drawing.rs:654`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L654)) already handles it as-is; no `formatScheme` is needed.

3. [`crates/pptx-render/src/layout.rs:761`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L761) - add `font_ref_color: Option<&'a ColorValue>` to
   `BodyCascade`, filled at the two construction sites (`:451` and `:571`) from
   `original` / `layout_node` / `master_node` via a `node_font_ref_color` helper written like
   `node_fill` ([`crates/pptx-render/src/layout.rs:1756`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1756)).

4. [`crates/pptx-render/src/layout.rs:808`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L808) - in `paragraph_properties`, install it as the base
   colour *after* the master `txStyles` seed and *before* the master/layout/primary body merge, so
   the run's own `rPr` and any `lstStyle`/`defRPr` still win. Gate it on `self.placeholder.is_none()`:
   for a real placeholder the `titleStyle`/`bodyStyle` seed is the correct list-style chain and must
   keep priority, whereas for a plain autoshape that seed is `otherStyle`, which does not belong in
   the chain at all (see the report) - replacing it with the `fontRef` colour is exactly right.

### B. Parse and apply the colour map

5. `crates/ooxml-drawingml/src/theme.rs` - add

   ```rust
   pub struct ColorMap { /* 12 slot -> theme-slot entries */ }
   ```

   whose `Default` is the standard master map (`text1 -> dk1`, `background1 -> lt1`,
   `text2 -> dk2`, `background2 -> lt2`, accents/links identity), plus
   `fn resolve<'a>(&'a self, slot: &'a str) -> &'a str`.

   Keep `normalize_scheme_color` ([`crates/pptx-parse/src/drawing.rs:695`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L695)) and
   `denormalize_scheme_color` ([`crates/pptx-parse/src/write.rs:1137`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1137)) exactly as they are: the
   `text1`/`background1`/... names they produce become the *map keys* rather than hardcoded
   aliases, so no serialized model or round trip changes. Keep the existing alias arms in
   `ThemeColorScheme::get` ([`crates/ooxml-drawingml/src/theme.rs:59`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L59)) as well - they then only
   serve the no-map path, which stays behaviour-identical.

6. [`crates/ooxml-drawingml/src/color.rs:61`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L61) - add
   `resolve_color_value_to_hex_with_map(color, theme, map)` and make the existing
   `resolve_color_value_to_hex_with_theme` a wrapper that passes `&ColorMap::default()`. docx,
   xlsx and every current pptx caller keep compiling unchanged.

7. `crates/pptx-parse/src/model.rs` - `color_map: ColorMap` on `SlideMaster` (`:102`),
   `color_map_override: Option<ColorMap>` on `SlideLayout` (`:90`) and `Slide` (`:78`), all
   `#[serde(default)]`. Populate in [`crates/pptx-parse/src/package.rs:75-128`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/package.rs#L75-L128) and in the slide loop
   (`:60-68`): `p:clrMap` on the master, `p:clrMapOvr/a:overrideClrMapping` on layout and slide,
   with `<a:masterClrMapping/>` parsing to `None`.

8. [`crates/pptx-render/src/layout.rs:317`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L317) - resolve the effective map once per slide
   (slide override, else layout override, else master map, else default) into a
   `color_map: ColorMap` field on `SlideRenderer`, and pass it at the six sites that reach a
   resolver: `paint` (`:1897`, used at `:183`, `:388`, `:526`), `stroke` (`:1930`, used at `:393`,
   `:530`, `:547`), `style_from_properties` (`:1849`) and `resolve_style` (`:1049`).

```rust
// crates/pptx-parse/src/drawing.rs, parse_shape
let style = element.child("style").map(|style| ShapeStyle {
    font_ref_color: style.child("fontRef").and_then(parse_color_container),
    font_ref: style
        .child("fontRef")
        .and_then(|value| value.attribute("idx"))
        .map(str::to_owned),
});

// crates/pptx-render/src/layout.rs, BodyCascade::paragraph_properties
let mut properties = self
    .master_slide
    .and_then(|master| master_style(master, self.placeholder, level))
    .cloned()
    .unwrap_or_default();
if self.placeholder.is_none()
    && let Some(color) = self.font_ref_color
{
    properties
        .default_run
        .get_or_insert_with(RunProperties::default)
        .color = Some(color.clone());
}
for body in [self.master, self.layout, self.primary].into_iter().flatten() { /* unchanged */ }

// crates/ooxml-drawingml/src/color.rs
pub fn resolve_color_value_to_hex_with_map(
    color: Option<&ColorValue>,
    theme: Option<&Theme>,
    map: &ColorMap,
) -> Option<String> {
    // ... same body, except:
    color.theme_color.as_deref().map(|slot| {
        let slot = map.resolve(slot);
        theme
            .and_then(|theme| theme.color_scheme.get(slot))
            .unwrap_or_else(|| default_theme_color(slot))
    })
}
```

Risks and tests to add:

- **The `phClr` / style-matrix gap stays open.** `ColorMap::resolve` must pass unknown names
  through untouched, and `default_theme_color` ([`crates/ooxml-drawingml/src/color.rs:183`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L183)) still
  turns anything it does not know into `000000`. Add a regression test that a `phClr` value is not
  made worse by the change.
- **Placeholders that also carry a `p:style`.** The `placeholder.is_none()` gate in step 4 is a
  deliberate simplification: PowerPoint applies `fontRef` under the list-style chain rather than
  skipping it. If a deck turns up where a placeholder's only colour source is its `fontRef`, the
  gate has to become "apply `fontRef` below the list style" instead, which needs the master
  `txStyles` seed and the shape's own `lstStyle` to be distinguishable - they are merged into one
  `ParagraphProperties` today.
- **A wider blast radius than the findings.** Step 8 changes shape fills, outlines, gradient stops
  and backgrounds as well as text, on every deck with a non-standard map. `rollout-plan` is the
  regression canary: its slide 1 background goes from grey to purple.
- **Charts.** `crates/pptx-render/src/chart.rs` and `crates/ooxml-drawingml/src/chart/` resolve
  their own colours; if they take the `..._with_theme` wrapper they silently keep the default map.
  That is correct only if no charted deck uses `clrMapOvr` - worth a grep before landing rather
  than an assumption.
- **Round trip.** Keeping `normalize_scheme_color`/`denormalize_scheme_color` untouched is what
  makes this safe; if a later cleanup removes the normalization, `ThemeColorScheme::get` must gain
  `tx1`/`bg1`/`tx2`/`bg2` arms in the same commit or every mapped colour goes black.
- **Tests to add** (none exist for either mechanism): a `pptx-parse` case asserting `p:style` and
  `p:clrMapOvr` reach the model; an `ooxml-drawingml` case that `tx1` resolves to `lt1` under an
  override and to `dk1` without one; a `pptx-render` case that an autoshape whose run has no
  `solidFill` takes its `fontRef` colour, and that a run-level `solidFill` still beats it.

**How to verify**

Re-render the four decks with `.venv/bin/python render-improvement-harness/scripts/render_bo.py`
then `diff.py`.

- `cisco-cloud-security` 04, 06, 09, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20, 23: every heading,
  digit and caption listed in the findings must come out `#FFFFFF`. Slide 6's diff (10.22%) should
  drop by the heading bands; the rest of it is `geometry-custom-collapses-to-bbox`.
- `project20` 03 and 04, `rollout-plan` 11: column headers and chevron labels come out `#FFFFFF`.
- `rollout-plan` 01: the largest single win in the cluster - the background alone accounts for the
  93.86% diff, which should collapse to whatever `picture-fill-fails-to-render` (the missing EMF)
  and the layout `lstStyle` size bug leave behind.
- `rollout-plan` 09: title `#FFFFFF`.
- `ocp-psp-plan` 14: expect *no* change from this fix alone; it needs the `lstStyle` and `gradFill`
  clusters first, after which the colour map decides white vs `#505050`.

No test in the tree covers either mechanism. [`crates/ooxml-drawingml/src/color.rs:200-295`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L200-L295) tests
`srgbClr`, tint/shade and the HSL modifiers but never a mapped name, and the `pptx-render` tests
from [`crates/pptx-render/src/layout.rs:2200`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2200) on build `TextStyle` values directly and never
exercise the cascade. Both need new tests rather than extended ones.

**Additional context**

none.

Related issues found in the same run: `geometry-custom-collapses-to-bbox`, `picture-fill-fails-to-render`, `text-inheritance-layout-lststyle-ignored`, `text-run-props-gradfill-not-resolved`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/package.rs`, `crates/pptx-parse/src/write.rs`, `crates/pptx-render/src/layout.rs`, `crates/ooxml-drawingml/src/theme.rs`, `crates/ooxml-drawingml/src/color.rs`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/theme-color-scheme-color-resolution-broken/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
