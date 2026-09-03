---
id: text-inheritance-layout-lststyle-ignored
title: Parse a:lstStyle and add it to the placeholder text-property cascade
category: text-inheritance
impact: high
effort: easy
confidence: high
status: open
occurrences: 14
decks: [ocp-psp-plan, project17, project20, rollout-plan]
findings: [ocp-psp-plan/13/1, ocp-psp-plan/14/1, project17/01/1, project17/04/1, project17/06/2, project20/02/1, project20/06/1, project20/08/2, project20/10/1, project20/15/1, project20/17/1, rollout-plan/01/2, rollout-plan/05/4, rollout-plan/09/2]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/pptx-render/src/layout.rs]
---

## Symptom

Placeholder text that carries no direct run formatting renders at the slide master's generic
`titleStyle`/`bodyStyle` defaults instead of the style the *layout's* placeholder shape declares in
its own `<a:txBody><a:lstStyle>`. Section titles come out roughly half size, unbold, in the master's
default text color, left-aligned instead of centred, and consequently wrap to the wrong number of
lines (evidence-1.png, evidence-2.png, evidence-3.png). The same gap makes a title-slide subtitle
render grey-on-dark instead of accent-coloured, effectively invisible (evidence-4.png, lower half).

The shape *box* is inherited correctly — the failing text sits in the right rectangle, only its type
is wrong. See "Not confirmed" below.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project20/08 | "CONVERSATION GUIDES" at 66pt bold navy wrapped to two lines (reference) vs. ~39pt regular grey on one line (candidate) — the layout's `sz="6600" b="1" srgbClr 24265D` is dropped |
| 2 | project20/17 | "THANK YOU" at 80pt bold centred (reference) vs. small regular left-aligned (candidate) — the layout's `sz="8000" b="1"` and `algn="ctr"` are dropped |
| 3 | rollout-plan/01 | title at the layout's 58.82pt over two lines (reference) vs. the master `titleStyle`'s 36pt on one line (candidate) |
| 4 | project17/01 | title-slide body placeholders: 40pt accent5 and 28pt accent6 (reference) vs. 20pt `tx2` grey for both (candidate); the date is nearly unreadable on the dark band |

## Root cause (hypothesis)

**Confirmed: `<a:lstStyle>` is never parsed anywhere in the pptx pipeline, so the shape-level tier of
the DrawingML text-property cascade does not exist.**

`parse_text_body` reads only `bodyPr` attributes and the `<a:p>` children —
`crates/pptx-parse/src/drawing.rs:764-789`. It never looks at the `<a:lstStyle>` sibling, and
`TextBody` (`crates/pptx-parse/src/model.rs:269-278`) has no field that could hold one. A repo-wide
grep for `lstStyle` finds exactly one other non-test hit, `crates/pptx-parse/src/write.rs:1769`,
which emits an *empty* `<a:lstStyle/>` when serializing a newly added shape. So the value is never
parsed — not parsed-and-dropped.

On the render side, `BodyCascade::paragraph_properties`
(`crates/pptx-render/src/layout.rs:808-827`) builds the effective paragraph properties as:

1. `master_style(...)` — the master's `p:txStyles` `titleStyle`/`bodyStyle`/`otherStyle` for the
   placeholder type and level (`crates/pptx-render/src/layout.rs:1786-1802`), then
2. `merge_paragraph_properties` from the master shape's, the layout shape's, and finally the slide
   shape's *paragraph* `pPr`, looked up by paragraph index (`layout.rs:814-826`).

Step 2 is the current stand-in for the missing tier, and it cannot work for these decks: the layout
placeholders' sample paragraphs are prompt text (`<a:p><a:pPr lvl="0"/><a:r><a:rPr lang="en-IN"/>
<a:t>SECTION TITLE</a:t></a:r></a:p>`) whose `pPr` carries nothing. All the formatting lives one
element up, in the `lstStyle` the parser discards.

The per-level properties do reach the runs once populated: `resolve_content` passes
`properties.default_run` into `resolve_style` (`crates/pptx-render/src/layout.rs:969-1000`), which is
where size / bold / family / colour fall back, and paragraph alignment falls back to
`properties.alignment` at `crates/pptx-render/src/layout.rs:995-1000`. So a correctly populated
`lstStyle` tier would land on exactly the properties that are wrong today, and
`merge_run_properties` (`layout.rs:1825-1847`) already merges every field an `lstStyle` `defRPr`
can set.

Confirmed against the XML for every deck in the cluster:

- `decks/project20/xml/08/layout.xml`, placeholder `idx="10"`:
  `<a:lstStyle><a:lvl1pPr marL="0" indent="0" algn="l">…<a:defRPr sz="6600" b="1"><a:solidFill>
  <a:srgbClr val="24265D"/></a:solidFill><a:latin typeface="+mn-lt"/></a:defRPr></a:lvl1pPr></a:lstStyle>`,
  against master `bodyStyle lvl1pPr defRPr sz="3921"` + `schemeClr tx1`. The slide shape is
  `<p:spPr/>` plus `<a:lstStyle/>` plus a run with a bare `<a:rPr lang="en-IN"/>`, so inheritance is
  the only possible source. The same layout and the same `idx="10"` back the project20/02, /06, /10
  and /15 findings.
- `decks/project20/xml/17/layout.xml`: `<a:lvl1pPr marL="0" indent="0" algn="ctr"><a:buNone/>
  <a:defRPr sz="8000" b="1">…`.
- `decks/rollout-plan/xml/01/layout.xml`, `ph type="title"`: `<a:lstStyle><a:lvl1pPr><a:defRPr
  sz="5882" spc="-98" …>` vs. master `titleStyle` `sz="3600"`. Slide 9 of the same deck uses the
  sibling layout with `sz="7058"`.
- `decks/rollout-plan/xml/05/layout.xml`, `idx="10"`: `<a:lvl2pPr marL="0" indent="0"><a:buNone/>
  <a:defRPr sz="1961"/></a:lvl2pPr>` — the `marL`/`indent` reset behind rollout-plan/05/4.
- `decks/project17/xml/01/layout.xml`: `idx="11"` → `sz="4000"` with `schemeClr accent5 lumMod 75000`;
  `idx="12"` → `sz="2800"` with `schemeClr accent6`; master `bodyStyle lvl1pPr` → `sz="2000"` with
  `schemeClr tx2`, which is exactly what the candidate renders.
- `decks/ocp-psp-plan/xml/13,14/layout.xml`: title `lstStyle lvl1pPr defRPr sz="6000"` vs. master
  `titleStyle sz="1962"`.

The same gap also covers project17/04/1's `TextBox 35/39/41`, which are *not* placeholders: they
carry their own `<a:lstStyle><a:lvl2pPr …><a:defRPr sz="1400">`. For a plain shape the `lstStyle` is
the shape's own primary tier, and `BodyCascade { primary: Some(body), layout: None, master: None }`
(`crates/pptx-render/src/layout.rs:570-577`) has nowhere to read it from either.

### Not confirmed

- **"position override ignored"** in the cluster symptom. `resolved_transform_value`
  (`crates/pptx-render/src/layout.rs:1860-1895`) already falls back to the layout node's, then the
  master node's, transform when the slide shape has no `xfrm`, and `find_placeholder` /
  `placeholders_match` (`layout.rs:1711-1733`) match `idx="10"` correctly. evidence-1.png and
  evidence-4.png show the candidate text sitting in the right box; what reads as "mispositioned" in
  the slide reports is smaller type inside a correctly placed `anchor="ctr"` box. project20/02/1's
  "position … ignored" claim is not reproduced.
- **Colour on rollout-plan/01/2 and /09/2.** Those layouts express the title colour as `gradFill`,
  and `parse_run_properties` only reads `solidFill` (`crates/pptx-parse/src/drawing.rs:917`). Their
  *size* is fixed by this issue; their colour additionally needs
  `text-run-props-gradfill-not-resolved` and the `clrMapOvr` theme-colour work.
- **project17/04/1's `Title 1`.** Its layout `lstStyle` is empty (`<a:lvl1pPr><a:defRPr/></a:lvl1pPr>`)
  and the master `titleStyle` says `sz="2000"`, yet the candidate renders *larger*, not smaller. That
  sub-finding has a different cause and is not explained by this issue; the `TextBox 35/39/41` half
  of the same finding is.
- `presentation.xml`'s `p:defaultTextStyle` is not parsed at all (no hits repo-wide). It is the tier
  below the master `txStyles`; out of scope here, but it is why non-placeholder shapes with no style
  of their own fall back to hard-coded renderer defaults.

## Verification

Re-render the 14 listed slides and check the four evidence slides first: project20/08 and /10 should
drop from 7.65% / 5.32% pixel diff to the residue of their other findings (the `grpFill` icon and the
alpha fill), project20/17 from 4.25% to near zero, project17/01 from 1.56% to near zero.
"CONVERSATION GUIDES" must wrap to two lines and paint `#24265D`; "THANK YOU" must be centred.

Unit coverage to add in the test module at `crates/pptx-render/src/layout.rs:2008`, alongside
`placeholder_matching_prefers_indices_and_normalizes_common_types` (`layout.rs:2490`): a synthetic
deck whose layout placeholder `lstStyle` sets `sz` / `b` / `algn` / `solidFill` while the master
`bodyStyle` sets different values, asserting the resolved `TextRun` takes the layout's. A parse-side
assertion that `parse_text_body` populates the new field belongs in `crates/pptx-parse`.

Existing tests most likely to move: `lays_out_demo_with_master_shapes_geometry_and_glyphs`
(`crates/pptx-render/src/layout.rs:2129`), `normal_autofit_scales_text_until_the_shape_height_is_respected`
(`layout.rs:2239`), and the raster goldens in `crates/pptx-raster/tests/golden.rs`, in particular
`golden_placeholder` (`golden.rs:328`). Any golden whose text sizes change has to be regenerated and
eyeballed — this change moves type metrics on every placeholder in every deck.
