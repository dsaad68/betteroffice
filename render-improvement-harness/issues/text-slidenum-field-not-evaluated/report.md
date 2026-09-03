---
id: text-slidenum-field-not-evaluated
title: Evaluate a:fld type="slidenum" instead of drawing its cached text
category: field-eval
impact: medium
effort: easy
confidence: high
status: open
occurrences: 13
decks: [cisco-cloud-security]
findings: [cisco-cloud-security/02/4, cisco-cloud-security/03/3, cisco-cloud-security/05/3, cisco-cloud-security/09/6, cisco-cloud-security/11/5, cisco-cloud-security/12/5, cisco-cloud-security/13/5, cisco-cloud-security/14/3, cisco-cloud-security/15/3, cisco-cloud-security/17/4, cisco-cloud-security/18/5, cisco-cloud-security/20/4, cisco-cloud-security/21/2]
files: [crates/pptx-render/src/layout.rs, crates/pptx-parse/src/package.rs, crates/pptx-parse/src/model.rs]
---

## Symptom

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

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/02 | reference prints `2`, candidate prints `‹#›` |
| 2 | cisco-cloud-security/11 | two-digit case: reference prints `11`, candidate prints `‹#›` |
| 3 | cisco-cloud-security/12 | reference prints `12`, candidate prints `‹#›` |
| 4 | cisco-cloud-security/15 and /05 | slide 15 fails identically; slide 5 (the "dingbat" finding) shows the same `‹#›`, disproving the font-substitution sub-claim |

## Root cause (confirmed)

`<a:fld>` is parsed as an ordinary run. `parse_text_paragraph` treats `fld` exactly like `r`
(`crates/pptx-parse/src/drawing.rs:823`) and `parse_text_run` records the field metadata into the
model (`crates/pptx-parse/src/drawing.rs:890` for `field_id`, `:893` for `field_type`), landing on
`TextRun::field_type` (`crates/pptx-parse/src/model.rs:332`).

Nothing ever reads it. `field_type` and `field_id` are written at those two sites and are not
referenced anywhere else in the workspace — grep for `field_type` across `crates/` returns only
the definition (`crates/pptx-parse/src/model.rs:332`) and the two assignments. The renderer
therefore draws the cached `<a:t>` unchanged: `content_from_body` copies `run.text` straight into
a `ContentRun` (`crates/pptx-render/src/layout.rs:884`, the `text: run.text.clone()` at `:897`),
dropping the field metadata that would have told it to substitute.

The affected shape reaches that function through the master/layout pass. `layout_slide` renders
master shapes that are *not* placeholders (`crates/pptx-render/src/layout.rs:208-218`) and layout
shapes likewise (`:219-229`), each via `render_parsed_shape`, which builds the text with
`content_from_body` at `crates/pptx-render/src/layout.rs:564`. `Rectangle 7` carries `<p:nvPr/>`
with no `<p:ph>`, so it passes the `node_placeholder(shape).is_none()` filter and is drawn on
every slide.

The slide number is available and unused: `layout_slide` already takes `slide_index: usize`
(`crates/pptx-render/src/layout.rs:132`) but `LayoutBuilder` (`:314-328`) does not carry it, so
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
(`crates/pptx-render/src/layout.rs:458`, `:862`) from a `TextRunSnapshot`
(`crates/pptx-edit/src/model.rs:58`) that has no field concept at all. `cisco-cloud-security` is
the only deck whose field lives on a master/layout as a plain shape, where the cached text is the
`‹#›` placeholder.

Three adjacent observations, all out of scope for this fix:

- The candidate also renders the footer at full opacity. The run's colour is
  `<a:srgbClr val="000000"><a:alpha val="25000"/></a:srgbClr>`, and the display list reports
  `color: "#000000"` — the opaque resolver (`crates/ooxml-drawingml/src/color.rs:89-101` documents
  that only the RGBA resolver keeps `a:alpha`) is used for text. That is the visible weight
  difference between the two rows of every evidence image and belongs with
  `fill-alpha-modifier-ignored`, not here.
- `p:presentation/@firstSlideNum` is not parsed (`parse_presentation`,
  `crates/pptx-parse/src/package.rs:232-278`, reads only `sldSz`, `sldIdLst` and
  `sldMasterIdLst`). This deck omits the attribute, so its default of 1 is correct here, but a
  deck that sets it will be off by a constant unless the fix reads it. Hypothesis, untested
  against a real deck.
- `datetimeFigureOut` fields exist on nine of the twelve decks' masters and layouts and have the
  same gap. They are all inside placeholders today, so none of them render; evaluating them needs
  a date policy and is a separate issue.

## Verification

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

Existing coverage is thin. `crates/pptx-edit/tests/write_fidelity.rs:43` embeds an
`<a:fld type="slidenum">` with cached `<a:t>1</a:t>`, asserts at `:432` that the saved slide still
contains `<a:fld `, and asserts at `:437` that the story's plain text is `"Hi Hello link\n1Accent"`
— i.e. the edit layer deliberately keeps the cached text as literal story content. That test pins
the constraint the fix must respect: substitution belongs on the render side, not in the story or
the writer. There is no test in `crates/pptx-render` or `crates/pptx-raster` that exercises a
field at all.
