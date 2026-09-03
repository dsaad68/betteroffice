---
id: text-bullets-autonum-not-drawn
title: Draw buAutoNum list numbers with a per-body, per-level counter
category: text-bullets
impact: medium
effort: medium
confidence: high
status: open
occurrences: 5
decks: [project20]
findings: [project20/11/1, project20/12/1, project20/13/1, project20/14/2, project20/16/2]
files: [crates/pptx-render/src/layout.rs, crates/pptx-parse/src/model.rs, crates/pptx-parse/src/drawing.rs]
---

## Symptom

Every paragraph carrying `<a:buAutoNum type="arabicPeriod"/>` renders with no number. The text
column itself is correct — first lines and wrapped continuation lines both sit at `marL`, exactly
where LibreOffice puts them — but the gutter to their left, where `1.` `2.` `3.` belong, is empty
(evidence-1.png, evidence-2.png). On slide 16 the same shape mixes level-0 `buAutoNum` with
level-1 `buChar`, and both markers vanish while the two indent steps survive (evidence-4.png).

This is the same downstream gap as `text-bullets-char-indent-dropped`. See that issue's report
for the shared analysis; this one covers only what `buAutoNum` needs on top of it — a counter, a
scheme formatter, and `buFont`.

The cluster's stated symptom "the hanging indent is sometimes preserved" is **not confirmed**;
it is preserved in all five findings. What is dropped is the `indent` attribute, which in these
paragraphs (`marL="514350" indent="-514350"`, identical on all five slides) positions only the
number, so its absence is invisible while no number is drawn.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project20/11 | `Rectangle 7`: `1.`–`5.` missing; the text column and the wrapped `integrate with?` continuation both land where LibreOffice puts them |
| 2 | project20/12 | eleven items: LibreOffice left-aligns `10.` and `11.` at the same gutter x as `1.`, so the marker is left-aligned at `marL + indent`, not right-aligned on the period |
| 3 | project20/13 | nine numbered questions, all unnumbered in the candidate; the multi-line items show the text hanging indent is intact |
| 4 | project20/16 | level-0 `buAutoNum` interleaved with six level-1 `buChar` paragraphs: LibreOffice numbers `1.` then resumes `2. 3. 4.` after the bullets, so the counter is per level and survives intervening paragraphs at another level |

## Root cause (hypothesis)

**The scheme and start value are parsed correctly and then discarded in layout.** Confirmed.

`buAutoNum` is parsed at `crates/pptx-parse/src/drawing.rs:860-866` into
`Bullet::AutoNumber { scheme, start_at }` (`crates/pptx-parse/src/model.rs:320-324`), defaulting
`scheme` to `arabicPeriod` and `start_at` to `1`, and stored on `ParagraphProperties::bullet`
(`crates/pptx-parse/src/drawing.rs:876`, `crates/pptx-parse/src/model.rs:305-312`). It survives
the render cascade: `BodyCascade::paragraph_properties`
(`crates/pptx-render/src/layout.rs:807-827`) merges master → layout → primary and
`merge_paragraph_properties` copies `bullet` field-wise
(`crates/pptx-render/src/layout.rs:1814-1816`).

It dies at `crates/pptx-render/src/layout.rs:994-1004`. `ResolvedParagraph`
(`crates/pptx-render/src/layout.rs:910-915`) has `align`, `level`, `margin_left_px` and `runs` —
no bullet, no indent. `layout_content` therefore only shifts the paragraph box right by `marL`
(`crates/pptx-render/src/layout.rs:1177-1178`) and `layout_paragraph`
(`crates/pptx-render/src/layout.rs:1192`) shapes nothing but the run text. Grepping `buAutoNum`
and `AutoNumber` across the workspace returns only the parse site, the write-back site
(`crates/pptx-parse/src/write.rs:1638-1645`) and the model — nothing in `pptx-render`,
`pptx-raster`, `ooxml-drawingml` or `ooxml-text` consumes it.

**Why this is not just "apply the buChar fix".** Three things `buChar` does not need:

1. *A counter.* The marker text depends on how many earlier paragraphs at the same level in the
   same text body were auto-numbered. `resolve_content` is called once per text body
   (`crates/pptx-render/src/layout.rs:666`) and walks paragraphs in order, so it is the natural
   home for a `[u32; 9]` counter — and it sits outside the autofit re-layout loop
   (`crates/pptx-render/src/layout.rs:686-698`), so the numbering is computed once no matter how
   many times `layout_content` re-runs.
2. *A scheme formatter.* `ST_TextAutonumberScheme` has ~23 members (`arabicPeriod`,
   `arabicParenR`, `alphaLcParenR`, `romanUcPeriod`, …), each a numeral style plus a suffix. The
   decks here only exercise `arabicPeriod`. Prior art exists on the docx side — `format_roman`
   (`crates/docx-edit/src/bridge/mod.rs:2156`), `format_alpha`
   (`crates/docx-edit/src/bridge/mod.rs:2174`) and `format_list_counter`
   (`crates/docx-edit/src/bridge/mod.rs:2190-2208`) — but `docx-edit` is not a dependency of
   `pptx-render`, so it has to be reimplemented or lifted into a shared crate.
3. *`buFont`.* All five findings carry `<a:buFont typeface="+mj-lt"/>`, which for this deck's
   theme resolves to `Segoe UI Light` against `Segoe UI` for the body text
   (`render-improvement-harness/decks/project20/xml/12/theme.xml`). `buFont`, `buClr` and
   `buSzPct` are not parsed anywhere (`grep buFont crates` returns nothing) and `Bullet` has no
   room for them (`crates/pptx-parse/src/model.rs:320-324`), so the number will inherit the run's
   face. A fidelity detail here, not the cause of the blank gutter.

**Marker placement.** Measured on `decks/project20/diff-img/12-sbs.png`, LibreOffice puts the
left edge of every marker glyph at the same x, `1.` through `11.` alike (evidence-2.png), ~51 px
left of the text column at 1280 px wide, against the 54 px that `marL - (marL + indent)` =
514350 EMU predicts — the 3 px difference being the left side bearing of the digit. So the
marker is left-aligned at `rect.x + marL + indent`, the same position the `buChar` solution
derives, and nothing about the text box moves.

**Not needed for this cluster:** the shape-level `<a:lstStyle>` parse gap from
`text-bullets-char-indent-dropped`. All five failing shapes are non-placeholder `Rectangle 7`s
with an empty `<a:lstStyle/>` and the `pPr` written on each `<a:p>`, so the property is already
reachable through `cascade.primary`.

**Unconfirmed:** what PowerPoint does when a shallower level increments after a deeper one has
been numbered (the docx implementation resets every deeper counter,
`crates/docx-edit/src/bridge/mod.rs:2311-2313`). Slide 16 only interleaves `buAutoNum` with
`buChar`, so these decks do not exercise it. Likewise `startAt` — no finding in this cluster
carries the attribute.

## Verification

Re-render `project20` slides 11, 12, 13, 14 and 16 with
`render-improvement-harness/scripts/render_bo.py` and re-run `diff.py`. The number column should
fill in and no text should move. Current fine diffs: `11` 4.50%, `12` 11.66%, `13` 13.93%,
`14` 3.86%, `16` 6.02%. Expect a couple of points off `12` and `13`; the rest of those two slides
is a separate line-height defect (`project20/13/4`, `project20/16/4`) and unapplied bold
(`project20/11/2`, `project20/12/2`, `project20/14/1`), which will keep the residual high.

No existing test covers bullets or auto-numbering: the `layout.rs` test module
(`crates/pptx-render/src/layout.rs:2008` onward) has no `marL`, bullet or `buAutoNum` case. Add
unit tests there for the counter across consecutive paragraphs, for a counter that resumes after
an intervening paragraph at another level (the slide-16 shape), and for `arabicPeriod` marker
text and placement at `rect.x + marL + indent`.
