---
id: text-bullets-char-indent-dropped
title: Draw buChar bullet glyphs and honour the paragraph hanging indent
category: text-bullets
impact: high
effort: medium
confidence: high
status: open
occurrences: 9
decks: [ocp-psp-plan, project17, project20, rollout-plan]
findings: [ocp-psp-plan/02/1, project17/05/3, project17/06/3, project17/07/2, project17/10/5, project20/04/5, rollout-plan/02/3, rollout-plan/05/3, rollout-plan/08/2]
files: [crates/pptx-render/src/layout.rs, crates/pptx-parse/src/model.rs, crates/pptx-parse/src/drawing.rs]
---

## Symptom

Every paragraph that carries a `buChar` bullet renders with no marker at all. The list text
itself lands in the right place — `marL` is applied, so first lines and wrapped continuation
lines sit where LibreOffice puts them — but the glyph column to their left is empty, so a
multi-level list collapses into an undifferentiated block of text (evidence-1.png: `•` at
level 1 and `–` at level 2 both vanish, while the two indent steps survive). The same happens
whether the bullet is declared on the paragraph's own `pPr` (evidence-3.png, evidence-4.png)
or inherited from the shape's `<a:lstStyle>` (evidence-2.png).

The cluster's original title also claims the hanging indent is dropped. That half is **not
confirmed**: in all four evidence slides the text columns line up with the reference to within
a pixel or two. What is genuinely dropped is the `indent` attribute, which in these decks only
governs where the *bullet* sits (it is `-marL`, or close to it, in every failing paragraph), so
its absence is invisible while no bullet is drawn at all.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project17/06 | `Rectangle 8`: level-1 `•` and level-2 `–` both missing; the two `marL` steps are still applied, so the text columns match the reference |
| 2 | project17/07 | `Rectangle 17`: the `▪` comes from the shape's own `<a:lstStyle>/<a:lvl2pPr>`, which is never parsed — the bullet is absent from the model, not just from the paint |
| 3 | rollout-plan/02 | `Rectangle 5`: bullet declared directly on `pPr` (`marL=342900 indent=-342900`); text starts at `marL` as it should, the `•` column is blank |
| 4 | rollout-plan/08 | `Rectangle 10`: wrapped continuation lines align with their first line in both renderers, showing the hanging indent is not the defect — only the glyph is |

## Root cause (hypothesis)

Two separate gaps, both confirmed against the XML.

**1. The bullet is parsed, cascaded, and then thrown away (7 of 9 findings).**

`buNone` / `buChar` / `buAutoNum` are parsed into `ParagraphProperties::bullet` at
`crates/pptx-parse/src/drawing.rs:853-867`, stored at `crates/pptx-parse/src/drawing.rs:876`,
with `marL` / `indent` alongside at `crates/pptx-parse/src/drawing.rs:874-875`. The render
cascade merges all three through the master → layout → slide chain
(`crates/pptx-render/src/layout.rs:808-828`, field-by-field merge at
`crates/pptx-render/src/layout.rs:1808-1816`).

The value dies at `crates/pptx-render/src/layout.rs:994-1004`: `ResolvedParagraph`
(`crates/pptx-render/src/layout.rs:910-915`) has exactly one geometry field, `margin_left_px`,
and no bullet field at all. `layout_content` therefore only shifts the paragraph box right by
`marL` (`crates/pptx-render/src/layout.rs:1177-1178`) and `layout_paragraph` shapes nothing but
the run text (`crates/pptx-render/src/layout.rs:1192-1265`). Grepping `bullet` across
`crates/pptx-render`, `crates/pptx-raster`, `crates/ooxml-drawingml` and `crates/ooxml-text`
returns only the two merge lines above — no consumer exists downstream. Parsed and ignored.

**2. A shape's own `<a:lstStyle>` is never parsed (project17/07/2, project17/10/5).**

`parse_text_body` (`crates/pptx-parse/src/drawing.rs:764-789`) reads `bodyPr` and the `<a:p>`
children and nothing else; `TextBody` (`crates/pptx-parse/src/model.rs:269-278`) has no
list-style field. In `decks/project17/xml/07/slide.xml` the `▪` for `Rectangle 17` lives in that
shape's `<a:lstStyle><a:lvl2pPr marL="193675" indent="-192088" …><a:buChar char="▪"/>`, and in
`decks/project17/xml/10/slide.xml` `TextBox 60` carries the same construct. For those two
findings the bullet is never parsed, so fixing (1) alone leaves them blank. This overlaps with
`text-inheritance-layout-lststyle-ignored`, which needs the same parse-side field for the
*layout* placeholder's `lstStyle`; landing the parse change once serves both.

Secondary, lower-confidence gap: `Bullet` (`crates/pptx-parse/src/model.rs:320-324`) carries
only the character, so `buFont`, `buClr` and `buSzPct` have nowhere to go. The failing decks all
use `buFont typeface="Arial"` with real Unicode characters and `buClr` values that resolve close
to the text colour, so this is a fidelity detail rather than the cause of the blank column — but
a bullet drawn in the run's own font at the run's own size will be visibly wrong for decks that
use Wingdings dingbats.

Painting is not the problem: `paint_lines` (`crates/pptx-raster/src/font.rs:47-68`) walks
`line.runs`, so a synthetic bullet run appended to the first line of each paragraph renders with
no raster change.

## Verification

Re-render `project17` slides 5, 6, 7, 10; `rollout-plan` slides 2, 5, 8; `ocp-psp-plan` slide 2;
`project20` slide 4 with `render-improvement-harness/scripts/render_bo.py` and re-run `diff.py`.
The bullet column should fill in and the text columns should not move. Expected drops:
`project17/06` 13.99% and `project17/07` 13.83% should fall by several points each;
`rollout-plan/02` 4.79% should drop toward the noise floor. `project20/04` will improve only
partly — that slide also loses its `spcBef`/`spcAft` paragraph gaps, a different defect.

No existing test covers bullets: the test module at `crates/pptx-render/src/layout.rs:2008`
onward has no bullet or `marL` case, and grepping `margin_left` across `crates/pptx-render`
finds only production code. A new unit test there should assert that a paragraph with
`marL`/`indent`/`buChar` produces a first line whose runs begin with the bullet glyph at
`rect.x + marL + indent`, that the text run still starts at `rect.x + marL`, and that
`caret_stops` are unchanged so hit testing (`crates/pptx-render/src/layout.rs:296`) does not
shift.
