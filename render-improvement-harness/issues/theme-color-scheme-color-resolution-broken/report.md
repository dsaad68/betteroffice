---
id: theme-color-scheme-color-resolution-broken
title: Parse p:style/a:fontRef and the clrMap/clrMapOvr colour map
category: theme-color
impact: high
effort: medium
confidence: high
status: open
occurrences: 20
decks: [cisco-cloud-security, ocp-psp-plan, project20, rollout-plan]
findings: [cisco-cloud-security/04/3, cisco-cloud-security/06/2, cisco-cloud-security/09/3, cisco-cloud-security/10/3, cisco-cloud-security/11/2, cisco-cloud-security/12/3, cisco-cloud-security/13/3, cisco-cloud-security/14/1, cisco-cloud-security/16/2, cisco-cloud-security/17/2, cisco-cloud-security/18/2, cisco-cloud-security/19/4, cisco-cloud-security/20/3, cisco-cloud-security/23/3, ocp-psp-plan/14/2, project20/03/1, project20/04/3, rollout-plan/01/1, rollout-plan/09/1, rollout-plan/11/1]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/pptx-parse/src/package.rs, crates/pptx-parse/src/write.rs, crates/pptx-render/src/layout.rs, crates/ooxml-drawingml/src/theme.rs, crates/ooxml-drawingml/src/color.rs]
---

## Symptom

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

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/06 | three `p:style/a:fontRef` `lt1` headings drawn `#676767`; on the `accent3` box the heading vanishes into its own fill |
| 2 | project20/03 | the same `fontRef` failure on four column headers and four body blocks, next to a title whose run-level `schemeClr bg1` *does* resolve to white - isolating the failure to `fontRef`, not to scheme colours in general |
| 3 | rollout-plan/01 | full slide: layout `clrMapOvr` ignored, so `bg2` paints `lt2` grey instead of `dk2` purple and the title's `tx1` paints `dk1` grey instead of `lt1` white |
| 4 | ocp-psp-plan/14 | the same layout `clrMapOvr` on a section divider; here the title lands on the hardcoded `#000000` fallback rather than on any theme colour |

## Root cause (confirmed)

### A. `p:style/a:fontRef` is never parsed (17 findings)

`fontRef` appears nowhere in the pptx crates - `grep -rn "fontRef\|fillRef\|lnRef\|effectRef" crates/`
returns only `crates/docx-parse/src/shape.rs`. `parse_shape`
(`crates/pptx-parse/src/drawing.rs:138-156`) reads `spPr`, `prstGeom`, fill, outline and `txBody`
and never looks at the sibling `p:style` element; `Shape`
(`crates/pptx-parse/src/model.rs:198-207`) has no field to put it in.

An autoshape's `p:style/a:fontRef` supplies the default text colour for every run in that shape
that does not set one. With it missing, `resolve_style`
(`crates/pptx-render/src/layout.rs:1042-1051`) falls through to the paragraph cascade, whose base
for a non-placeholder shape is the master's `p:otherStyle`
(`crates/pptx-render/src/layout.rs:808-812`, `:1786-1801`) - and every deck in this cluster sets
that to `schemeClr tx1`.

Confirmed on `cisco-cloud-security` slide 6, id 46:

```xml
<a:solidFill><a:schemeClr val="accent3"/></a:solidFill>        <!-- 676767 -->
<p:style>...<a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style>
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
`crates/pptx-render/src/layout.rs:1051`.

### B. `p:clrMap` / `p:clrMapOvr` are never parsed (3 findings)

The colour map is not in the model: `SlideMaster` (`crates/pptx-parse/src/model.rs:102-110`),
`SlideLayout` (`:90-99`) and `Slide` (`:78-86`) carry no map field, and neither
`common_slide_data` (`crates/pptx-parse/src/drawing.rs:41-63`) nor the master/layout loops in
`crates/pptx-parse/src/package.rs:75-128` read `p:clrMap` or `p:clrMapOvr`. The only occurrence in
the tree is on the write path (`crates/pptx-parse/src/write.rs:1832`), which emits a hardcoded
`<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>` for synthesized slides.

Instead the standard map is baked in at parse time. `normalize_scheme_color`
(`crates/pptx-parse/src/drawing.rs:695-703`) rewrites `tx1`->`text1`, `bg1`->`background1`,
`tx2`->`text2`, `bg2`->`background2`, and `ThemeColorScheme::get`
(`crates/ooxml-drawingml/src/theme.rs:59-74`) then reads `text1` as `dk1` and `background1` as
`lt1`. That is exactly the master's default `clrMap`, and it cannot be overridden.

Confirmed on `rollout-plan` slide 1:

```xml
<!-- slideMaster1.xml -->
<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" .../>
<!-- slideLayout4.xml -->
<p:clrMapOvr><a:overrideClrMapping bg1="dk1" tx1="lt1" bg2="dk2" tx2="lt2" .../></p:clrMapOvr>
<p:bg><p:bgRef idx="1001"><a:schemeClr val="bg2"/></p:bgRef></p:bg>
<!-- slideMaster1.xml titleStyle/lvl1pPr/defRPr -->
<a:solidFill><a:schemeClr val="tx1"/></a:solidFill>
```

with theme `dk1 = 505050`, `lt1 = FFFFFF`, `dk2 = 68217A`, `lt2 = D2D2D2`. Under the override the
background must be `#68217A` and the title `#FFFFFF`; the renderer produces `#D2D2D2` and,
measured from the display list at HEAD, `'Change Management Roll-out Plan ' -> #505050`.
`rollout-plan` slide 9 uses the same layout family and yields `'Appendix' -> #505050` where
`#FFFFFF` is required.

A landmine for whoever fixes this: `ThemeColorScheme::get`
(`crates/ooxml-drawingml/src/theme.rs:59-74`) has no arm for the raw `tx1`/`bg1`/`tx2`/`bg2` names
(nor for `phClr`), and `default_theme_color` (`crates/ooxml-drawingml/src/color.rs:183-199`) sends
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
  `crates/pptx-render/src/layout.rs:1051`. Fixing the colour map is necessary but not sufficient
  for this slide. `rollout-plan/09/1` has the same `gradFill` construct in its master `titleStyle`,
  though there a `tx1` colour does reach the run.
- **`project20/03/1`'s claim that an explicit run-level `schemeClr` also fails is wrong.** The
  "Workstreams" title's `<a:schemeClr val="bg1"/>` resolves correctly to `#FFFFFF` (evidence-2.png,
  and measured from the display list). Only the `fontRef`-coloured shapes on that slide fail.
- The style matrix behind `fillRef`/`lnRef`/`effectRef` is a *separate* problem: `Theme`
  (`crates/ooxml-drawingml/src/theme.rs:126-131`) has no `formatScheme`, so those three references
  cannot be resolved at all. `fontRef` does not need it - its colour is a direct child of the
  element, and `idx="minor"`/`"major"` maps onto the font scheme that is already parsed.

## Verification

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

No test in the tree covers either mechanism. `crates/ooxml-drawingml/src/color.rs:200-295` tests
`srgbClr`, tint/shade and the HSL modifiers but never a mapped name, and the `pptx-render` tests
from `crates/pptx-render/src/layout.rs:2200` on build `TextStyle` values directly and never
exercise the cascade. Both need new tests rather than extended ones.
