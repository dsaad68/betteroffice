# pptx: Layout placeholder's own lstStyle/position override ignored

**Describe the bug**

Placeholder text that carries no direct run formatting renders at the slide master's generic
`titleStyle`/`bodyStyle` defaults instead of the style the *layout's* placeholder shape declares in
its own `<a:txBody><a:lstStyle>`. Section titles come out roughly half size, unbold, in the master's
default text color, left-aligned instead of centred, and consequently wrap to the wrong number of
lines (evidence-1.png, evidence-2.png, evidence-3.png). The same gap makes a title-slide subtitle
render grey-on-dark instead of accent-coloured, effectively invisible (evidence-4.png, lower half).

The shape *box* is inherited correctly — the failing text sits in the right rectangle, only its type
is wrong. See "Not confirmed" below.

Seen on 14 slides across 4 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project20/08** "CONVERSATION GUIDES" at 66pt bold navy wrapped to two lines (reference) vs. ~39pt regular grey on one line (candidate) — the layout's `sz="6600" b="1" srgbClr 24265D` is dropped

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-inheritance-layout-lststyle-ignored/evidence-1.png)

**2. project20/17** "THANK YOU" at 80pt bold centred (reference) vs. small regular left-aligned (candidate) — the layout's `sz="8000" b="1"` and `algn="ctr"` are dropped

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-inheritance-layout-lststyle-ignored/evidence-2.png)

**3. rollout-plan/01** title at the layout's 58.82pt over two lines (reference) vs. the master `titleStyle`'s 36pt on one line (candidate)

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-inheritance-layout-lststyle-ignored/evidence-3.png)

**4. project17/01** title-slide body placeholders: 40pt accent5 and 28pt accent6 (reference) vs. 20pt `tx2` grey for both (candidate); the date is nearly unreadable on the dark band

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-inheritance-layout-lststyle-ignored/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 13, 14
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 4, 6
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 6, 8, 10, 15, 17
- `rollout-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 5, 9

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(12, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**Confirmed: `<a:lstStyle>` is never parsed anywhere in the pptx pipeline, so the shape-level tier of
the DrawingML text-property cascade does not exist.**

`parse_text_body` reads only `bodyPr` attributes and the `<a:p>` children —
[`crates/pptx-parse/src/drawing.rs:764-789`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L764-L789). It never looks at the `<a:lstStyle>` sibling, and
`TextBody` ([`crates/pptx-parse/src/model.rs:269-278`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L269-L278)) has no field that could hold one. A repo-wide
grep for `lstStyle` finds exactly one other non-test hit, [`crates/pptx-parse/src/write.rs:1769`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1769),
which emits an *empty* `<a:lstStyle/>` when serializing a newly added shape. So the value is never
parsed — not parsed-and-dropped.

On the render side, `BodyCascade::paragraph_properties`
([`crates/pptx-render/src/layout.rs:808-827`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L808-L827)) builds the effective paragraph properties as:

1. `master_style(...)` — the master's `p:txStyles` `titleStyle`/`bodyStyle`/`otherStyle` for the
   placeholder type and level ([`crates/pptx-render/src/layout.rs:1786-1802`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1786-L1802)), then
2. `merge_paragraph_properties` from the master shape's, the layout shape's, and finally the slide
   shape's *paragraph* `pPr`, looked up by paragraph index (`layout.rs:814-826`).

Step 2 is the current stand-in for the missing tier, and it cannot work for these decks: the layout
placeholders' sample paragraphs are prompt text (`<a:p><a:pPr lvl="0"/><a:r><a:rPr lang="en-IN"/>
<a:t>SECTION TITLE</a:t></a:r></a:p>`) whose `pPr` carries nothing. All the formatting lives one
element up, in the `lstStyle` the parser discards.

The per-level properties do reach the runs once populated: `resolve_content` passes
`properties.default_run` into `resolve_style` ([`crates/pptx-render/src/layout.rs:969-1000`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L969-L1000)), which is
where size / bold / family / colour fall back, and paragraph alignment falls back to
`properties.alignment` at [`crates/pptx-render/src/layout.rs:995-1000`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L995-L1000). So a correctly populated
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
([`crates/pptx-render/src/layout.rs:570-577`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L570-L577)) has nowhere to read it from either.

### Not confirmed

- **"position override ignored"** in the cluster symptom. `resolved_transform_value`
  ([`crates/pptx-render/src/layout.rs:1860-1895`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1860-L1895)) already falls back to the layout node's, then the
  master node's, transform when the slide shape has no `xfrm`, and `find_placeholder` /
  `placeholders_match` (`layout.rs:1711-1733`) match `idx="10"` correctly. evidence-1.png and
  evidence-4.png show the candidate text sitting in the right box; what reads as "mispositioned" in
  the slide reports is smaller type inside a correctly placed `anchor="ctr"` box. project20/02/1's
  "position … ignored" claim is not reproduced.
- **Colour on rollout-plan/01/2 and /09/2.** Those layouts express the title colour as `gradFill`,
  and `parse_run_properties` only reads `solidFill` ([`crates/pptx-parse/src/drawing.rs:917`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L917)). Their
  *size* is fixed by this issue; their colour additionally needs
  `text-run-props-gradfill-not-resolved` and the `clrMapOvr` theme-colour work.
- **project17/04/1's `Title 1`.** Its layout `lstStyle` is empty (`<a:lvl1pPr><a:defRPr/></a:lvl1pPr>`)
  and the master `titleStyle` says `sz="2000"`, yet the candidate renders *larger*, not smaller. That
  sub-finding has a different cause and is not explained by this issue; the `TextBox 35/39/41` half
  of the same finding is.
- `presentation.xml`'s `p:defaultTextStyle` is not parsed at all (no hits repo-wide). It is the tier
  below the master `txStyles`; out of scope here, but it is why non-placeholder shapes with no style
  of their own fall back to hard-coded renderer defaults.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Add the missing shape-level tier to the cascade. Two small changes:

**Parse.** Give `TextBody` a `list_style: Vec<ParagraphProperties>` field
([`crates/pptx-parse/src/model.rs:269`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L269)) and fill it in `parse_text_body`
([`crates/pptx-parse/src/drawing.rs:764`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L764)). No new parser is needed:
`parse_style_levels` ([`crates/pptx-parse/src/drawing.rs:78`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L78)) already walks `lvl1pPr`..`lvl9pPr`
children into a 9-slot `Vec<ParagraphProperties>` and returns an empty vec when there are none, which
is exactly the `lstStyle` content model and exactly the no-op an empty `<a:lstStyle/>` needs. Mark the
field `#[serde(default, skip_serializing_if = "Vec::is_empty")]` so existing snapshot JSON keeps
round-tripping.

**Render.** In `BodyCascade::paragraph_properties`
([`crates/pptx-render/src/layout.rs:808`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L808)), interleave each body's `list_style[level]` ahead of that
body's paragraph `pPr`, walking master → layout → primary. The resulting order is the ECMA-376 one:
master `txStyles` → master shape `lstStyle` → layout shape `lstStyle` → shape's own `lstStyle` →
paragraph `pPr`. Keeping the existing paragraph-`pPr` merges in place makes this additive rather than
a rewrite of the cascade.

Index by `level` only (no `.or_else(first)` fallback like `master_style` uses): an `lstStyle` that
defines only `lvl1pPr` must contribute nothing to a level-3 paragraph.

This also covers non-placeholder shapes such as project17/04's `TextBox 35/39/41`, because their
`lstStyle` arrives through `primary` in the `BodyCascade { primary: Some(body), layout: None,
master: None }` built at [`crates/pptx-render/src/layout.rs:570`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L570).

```rust
// crates/pptx-parse/src/model.rs
pub struct TextBody {
    // …
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list_style: Vec<ParagraphProperties>,
    pub paragraphs: Vec<TextParagraph>,
}

// crates/pptx-parse/src/drawing.rs, in parse_text_body
Ok(TextBody {
    // …
    list_style: parse_style_levels(element.child("lstStyle")),
    paragraphs,
})

// crates/pptx-render/src/layout.rs, BodyCascade::paragraph_properties
let mut properties = self
    .master_slide
    .and_then(|master| master_style(master, self.placeholder, level))
    .cloned()
    .unwrap_or_default();
for body in [self.master, self.layout, self.primary].into_iter().flatten() {
    if let Some(source) = body.list_style.get(level as usize) {
        merge_paragraph_properties(&mut properties, source);
    }
    if let Some(source) = body
        .paragraphs
        .get(index)
        .or_else(|| body.paragraphs.get(level as usize))
        .map(|paragraph| &paragraph.properties)
    {
        merge_paragraph_properties(&mut properties, source);
    }
}
properties
```

`merge_paragraph_properties` (`layout.rs:1804`) and `merge_run_properties` (`layout.rs:1825`) already
cover `algn`, `marL`, `indent`, `buNone`/`buChar`, and every `defRPr` field these decks use, so no
merge logic changes.

Risks and tests to add:

- **Wide blast radius on text metrics.** Every placeholder in every deck gains a style tier it did
  not have. Expect the raster goldens to move — regenerate and eyeball
  `crates/pptx-raster/tests/golden.rs`, `golden_placeholder` (`golden.rs:328`) especially — plus
  `lays_out_demo_with_master_shapes_geometry_and_glyphs` (`layout.rs:2129`) and
  `normal_autofit_scales_text_until_the_shape_height_is_respected` (`layout.rs:2239`), where larger
  resolved sizes will change the autofit scale that gets reached.
- **Correctly larger text will now overflow.** Several of these placeholders declare `<a:noAutofit/>`
  (project20's `idx="10"`), so text that grows from 39pt to 66pt is *meant* to overflow or rewrap.
  Do not tune the fix by the total pixel diff alone; check the wrap points named in the report.
- **Ordering against the existing paragraph-`pPr` hack.** Interleaving as sketched lets a layout's
  sample-paragraph `pPr` override the same layout's `lstStyle`, which PowerPoint would not do. It is
  harmless on these decks (those `pPr`s are empty) but is the thing to revisit if a regression shows
  up; the real fix there is to stop treating layout/master sample paragraphs as a style source at all.
- **New model field crosses the wasm and Python boundaries** via `crates/betteroffice-pptx/src/types.rs`
  and the snapshot serde; `skip_serializing_if` keeps the emitted JSON unchanged for decks with no
  `lstStyle`, but run the `pptx-edit` write-fidelity tests
  (`crates/pptx-edit/tests/write_fidelity.rs`) to confirm the round-trip is untouched.
- **Does not fix colour on rollout-plan.** Those `lstStyle` `defRPr`s use `gradFill`, which
  `parse_run_properties` ([`crates/pptx-parse/src/drawing.rs:917`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L917)) does not read. Sizes will be
  correct, colours still need the `gradFill` and `clrMapOvr` issues.

**How to verify**

Re-render the 14 listed slides and check the four evidence slides first: project20/08 and /10 should
drop from 7.65% / 5.32% pixel diff to the residue of their other findings (the `grpFill` icon and the
alpha fill), project20/17 from 4.25% to near zero, project17/01 from 1.56% to near zero.
"CONVERSATION GUIDES" must wrap to two lines and paint `#24265D`; "THANK YOU" must be centred.

Unit coverage to add in the test module at [`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2008), alongside
`placeholder_matching_prefers_indices_and_normalizes_common_types` (`layout.rs:2490`): a synthetic
deck whose layout placeholder `lstStyle` sets `sz` / `b` / `algn` / `solidFill` while the master
`bodyStyle` sets different values, asserting the resolved `TextRun` takes the layout's. A parse-side
assertion that `parse_text_body` populates the new field belongs in `crates/pptx-parse`.

Existing tests most likely to move: `lays_out_demo_with_master_shapes_geometry_and_glyphs`
([`crates/pptx-render/src/layout.rs:2129`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2129)), `normal_autofit_scales_text_until_the_shape_height_is_respected`
(`layout.rs:2239`), and the raster goldens in `crates/pptx-raster/tests/golden.rs`, in particular
`golden_placeholder` (`golden.rs:328`). Any golden whose text sizes change has to be regenerated and
eyeballed — this change moves type metrics on every placeholder in every deck.

**Additional context**

none.

Related issues found in the same run: `text-run-props-gradfill-not-resolved`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-render/src/layout.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-inheritance-layout-lststyle-ignored/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
