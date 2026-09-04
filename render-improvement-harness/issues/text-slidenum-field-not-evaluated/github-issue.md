# pptx: Slide-number field renders literal placeholder glyph

**Describe the bug**

Every slide in `cisco-cloud-security` prints the literal string `‹#›` in the bottom-right footer
where the reference prints the slide's number (evidence-1.png, evidence-2.png, evidence-3.png).
The string is byte-identical on all 23 slides, so the deck has no page numbers at all rather than
wrong ones.

The failing text comes from a plain (non-placeholder) master shape named `Rectangle 7` whose only
run is `<a:fld type="slidenum">`. Its cached `<a:t>` is the PowerPoint placeholder glyph `‹#›`,
which BetterOffice draws verbatim.

Not confirmed: the cluster symptom also claims one instance renders through "a dingbat-like font
substitution" (finding `cisco-cloud-security/05/3`). Slide 5 renders exactly the same `‹#›` at the
same 6 pt size as every other slide (evidence-4.png, lower pair) — the glyphs only look like
dingbats because `‹`, `#` and `›` at 6 pt on a 96 dpi raster are about eight pixels tall. There is
no font-substitution defect here.

Seen on 13 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/02** reference prints `2`, candidate prints `‹#›`

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-slidenum-field-not-evaluated/evidence-1.png)

**2. cisco-cloud-security/11** two-digit case: reference prints `11`, candidate prints `‹#›`

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-slidenum-field-not-evaluated/evidence-2.png)

**3. cisco-cloud-security/12** reference prints `12`, candidate prints `‹#›`

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-slidenum-field-not-evaluated/evidence-3.png)

**4. cisco-cloud-security/15 and /05** slide 15 fails identically; slide 5 (the "dingbat" finding) shows the same `‹#›`, disproving the font-substitution sub-claim

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-slidenum-field-not-evaluated/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 3, 5, 9, 11, 12, 13, 14, 15, 17, 18, 20, 21

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(1, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

`<a:fld>` is parsed as an ordinary run. `parse_text_paragraph` treats `fld` exactly like `r`
([`crates/pptx-parse/src/drawing.rs:823`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L823)) and `parse_text_run` records the field metadata into the
model ([`crates/pptx-parse/src/drawing.rs:890`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L890) for `field_id`, `:893` for `field_type`), landing on
`TextRun::field_type` ([`crates/pptx-parse/src/model.rs:332`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L332)).

Nothing ever reads it. `field_type` and `field_id` are written at those two sites and are not
referenced anywhere else in the workspace — grep for `field_type` across `crates/` returns only
the definition ([`crates/pptx-parse/src/model.rs:332`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L332)) and the two assignments. The renderer
therefore draws the cached `<a:t>` unchanged: `content_from_body` copies `run.text` straight into
a `ContentRun` ([`crates/pptx-render/src/layout.rs:884`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L884), the `text: run.text.clone()` at `:897`),
dropping the field metadata that would have told it to substitute.

The affected shape reaches that function through the master/layout pass. `layout_slide` renders
master shapes that are *not* placeholders ([`crates/pptx-render/src/layout.rs:208-218`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L208-L218)) and layout
shapes likewise (`:219-229`), each via `render_parsed_shape`, which builds the text with
`content_from_body` at [`crates/pptx-render/src/layout.rs:564`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L564). `Rectangle 7` carries `<p:nvPr/>`
with no `<p:ph>`, so it passes the `node_placeholder(shape).is_none()` filter and is drawn on
every slide.

The slide number is available and unused: `layout_slide` already takes `slide_index: usize`
([`crates/pptx-render/src/layout.rs:132`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L132)) but `LayoutBuilder` (`:314-328`) does not carry it, so
`content_from_body` has nothing to substitute with.

Confirmed by experiment, not by reading alone. Rendering the deck through `bindings/python-pptx`
and scanning the display list finds the same literal on all 23 slides:

```python
d = bo.Presentation.open_path("render-improvement-harness/decks/cisco-cloud-security/source.pptx")
register_fonts(d)
for i in range(len(d)):
    for p in d.render_slide(i).to_dict()["primitives"]:
        text = "".join(r["text"] for l in p.get("lines", []) for r in l.get("runs", []))
        if "‹#›" in text:
            print(i + 1, repr(text))   # prints 1..23, always '‹#›'
```

Why only this deck. Of the twelve harness decks, ten put their `slidenum` field inside a `sldNum`
placeholder on the master or layout, which `layout_slide` skips at `:210` / `:221`, so no field is
drawn at all. The three decks with slide-level `<a:fld type="slidenum">` (`ocp-psp-plan`,
`project17`, `rollout-plan`) render correctly *by accident*: PowerPoint writes the evaluated value
into the slide-level cached `<a:t>` (`ppt/slides/slide15.xml` → `<a:t>15</a:t>`, and so on for all
20 slide-level fields in those decks), and that path goes through `content_from_story`
([`crates/pptx-render/src/layout.rs:458`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L458), `:862`) from a `TextRunSnapshot`
([`crates/pptx-edit/src/model.rs:58`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L58)) that has no field concept at all. `cisco-cloud-security` is
the only deck whose field lives on a master/layout as a plain shape, where the cached text is the
`‹#›` placeholder.

Three adjacent observations, all out of scope for this fix:

- The candidate also renders the footer at full opacity. The run's colour is
  `<a:srgbClr val="000000"><a:alpha val="25000"/></a:srgbClr>`, and the display list reports
  `color: "#000000"` — the opaque resolver ([`crates/ooxml-drawingml/src/color.rs:89-101`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L89-L101) documents
  that only the RGBA resolver keeps `a:alpha`) is used for text. That is the visible weight
  difference between the two rows of every evidence image and belongs with
  `fill-alpha-modifier-ignored`, not here.
- `p:presentation/@firstSlideNum` is not parsed (`parse_presentation`,
  [`crates/pptx-parse/src/package.rs:232-278`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/package.rs#L232-L278), reads only `sldSz`, `sldIdLst` and
  `sldMasterIdLst`). This deck omits the attribute, so its default of 1 is correct here, but a
  deck that sets it will be off by a constant unless the fix reads it. Hypothesis, untested
  against a real deck.
- `datetimeFigureOut` fields exist on nine of the twelve decks' masters and layouts and have the
  same gap. They are all inside placeholders today, so none of them render; evaluating them needs
  a date policy and is a separate issue.

**Suggested fix**

Evaluate the field where the display list is built, and only there. The edit layer must keep
treating a field's cached `<a:t>` as literal story text — [`crates/pptx-edit/tests/write_fidelity.rs:437`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/tests/write_fidelity.rs#L437)
asserts exactly that, and the writer round-trips `<a:fld>` through `is_run_element`
([`crates/pptx-parse/src/write.rs:1339`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1339)) on the same assumption. Substituting at parse or story
level would break both.

Three small changes in `crates/pptx-render/src/layout.rs`:

1. Carry the number. `LayoutBuilder` (`:314`) gains `slide_number: u32`, filled in
   `layout_slide` (`:128`) from `slide_index + 1`, offset by the deck's first slide number.
2. Substitute in `content_from_body` (`:884`). It already iterates `pptx_parse::TextRun`, which
   carries `field_type` ([`crates/pptx-parse/src/model.rs:332`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L332)); give the function the slide number
   and map a `slidenum` run's text to the decimal number instead of `run.text.clone()` (`:897`).
   The single call site is `:564`.
3. Read `p:presentation/@firstSlideNum` in `parse_presentation`
   ([`crates/pptx-parse/src/package.rs:232`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/package.rs#L232)) into a new `Presentation::first_slide_num: u32`
   (default 1), so the displayed number is `first_slide_num + slide_index`. Optional — every
   harness deck omits the attribute — but it is four lines and avoids a known off-by-N.

Deliberately out of scope: the `content_from_story` path (`:862`). Slide-level fields already
carry the evaluated number in their cached text, `TextRunSnapshot` ([`crates/pptx-edit/src/model.rs:58`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L58))
has no field concept, and threading one through the Yjs story model is a much larger change with
its own edit-semantics questions (what happens when a user types inside a field). Note it in the
code so the asymmetry is not read as an oversight.

`crates/pptx-render/src/lib.rs`'s `ComposedRun` (host-composed JSON) has no field concept either;
hosts on that path pre-resolve their own text, so it needs nothing.

```rust
// layout.rs
struct LayoutBuilder<'a> {
    // ...
    slide_number: u32,
}

// in layout_slide, next to the other builder fields:
slide_number: package.presentation.first_slide_num
    .saturating_add(u32::try_from(slide_index).unwrap_or(u32::MAX)),

// at the single call site (:564)
let content = content_from_body(stable_id, body, self.theme, self.slide_number);

fn content_from_body(
    story_id: &str,
    body: &TextBody,
    theme: &Theme,
    slide_number: u32,
) -> TextContent {
    // ...
    .map(|run| ContentRun {
        text: match run.field_type.as_deref() {
            // Slide-level fields cache the evaluated value already and arrive
            // through content_from_story, which has no field metadata.
            Some("slidenum") => slide_number.to_string(),
            _ => run.text.clone(),
        },
        style: style_from_properties(&run.properties, theme),
    })
}
```

```rust
// package.rs, in parse_presentation
first_slide_num: root
    .attribute("firstSlideNum")
    .and_then(|value| value.parse::<u32>().ok())
    .unwrap_or(1),
```

Risks and tests to add:

- Any master or layout that draws a non-placeholder `slidenum` field changes text, so its shaped
  width changes. The cisco shape is `wrap="none"` + `<a:spAutoFit/>` + `algn="r"`, so a shorter
  string moves the left edge, not the right one, and the number stays anchored where the reference
  puts it. A `algn="l"` or `algn="ctr"` field would shift; nothing in the harness decks does this,
  so it is untested.
- Adding a field to `Presentation` touches the public parse model; check
  [`crates/pptx-parse/src/write.rs:563`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L563) region, which rebuilds `sldIdLst`, still round-trips the
  attribute untouched (it should — the writer patches XML rather than regenerating it, but the
  round-trip tests in `crates/pptx-edit/tests/write_fidelity.rs` are the place to confirm).
- Golden images: any pptx golden whose master carries a plain `slidenum` shape will change.
  Check `crates/pptx-raster/tests/golden` before regenerating.
- Do not "fix" this by substituting in `parse_text_run` — that would write the evaluated number
  back on save and corrupt the deck for every other consumer.

Tests to add:

- A unit test in the `layout.rs` test module: a master with a non-placeholder shape whose run is
  `<a:fld type="slidenum"><a:t>‹#›</a:t></a:fld>`, laid out at `slide_index` 0 and 2, produces
  text primitives reading `1` and `3`.
- The same deck with `firstSlideNum="7"` produces `7` and `9`.
- A non-`slidenum` field (`datetimeFigureOut`) still renders its cached text unchanged, pinning
  the deliberate scope.
- A regression assertion that `DeckSession::save()` on a deck with a `slidenum` field still
  emits the original `‹#›` cached text — extend `crates/pptx-edit/tests/write_fidelity.rs`.

**How to verify**

The failing glyphs are a 6 pt string in a ~20 x 8 px box, roughly 0.03% of the slide, so
`fine_pct` will not move measurably — slide 15 is at 26.91% and slide 12 at 21.27%, both dominated
by other clusters. Do not use the diff percentage as the acceptance signal.

Check the display list instead: after the fix the snippet above must print `1`..`23` with the
matching digits and find no `‹#›` anywhere. Then re-render and eyeball the same crop:

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py cisco-cloud-security
.venv/bin/python render-improvement-harness/scripts/diff.py cisco-cloud-security
```

No slide should regress. `render_bo.py` registers Liberation Sans for `Arial`, which is what
`+mn-lt` resolves to here, so the digits will be metrically close to the reference.

Existing coverage is thin. [`crates/pptx-edit/tests/write_fidelity.rs:43`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/tests/write_fidelity.rs#L43) embeds an
`<a:fld type="slidenum">` with cached `<a:t>1</a:t>`, asserts at `:432` that the saved slide still
contains `<a:fld `, and asserts at `:437` that the story's plain text is `"Hi Hello link\n1Accent"`
— i.e. the edit layer deliberately keeps the cached text as literal story content. That test pins
the constraint the fix must respect: substitution belongs on the render side, not in the story or
the writer. There is no test in `crates/pptx-render` or `crates/pptx-raster` that exercises a
field at all.

**Additional context**

none.

Related issues found in the same run: `fill-alpha-modifier-ignored`

Files most likely involved: `crates/pptx-render/src/layout.rs`, `crates/pptx-parse/src/package.rs`, `crates/pptx-parse/src/model.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-slidenum-field-not-evaluated/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
