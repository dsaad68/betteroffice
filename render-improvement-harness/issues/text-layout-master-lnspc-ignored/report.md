---
id: text-layout-master-lnspc-ignored
title: Parse and apply paragraph lnSpc, and stop clamping the vertical anchor on overflow
category: text-layout
impact: medium
effort: medium
confidence: high
status: open
occurrences: 7
decks: [cisco-cloud-security]
findings: [cisco-cloud-security/04/4, cisco-cloud-security/06/3, cisco-cloud-security/08/4, cisco-cloud-security/11/4, cisco-cloud-security/15/1, cisco-cloud-security/16/1, cisco-cloud-security/18/4]
files: [crates/pptx-parse/src/model.rs, crates/pptx-parse/src/drawing.rs, crates/pptx-render/src/layout.rs]
---

## Symptom

Every two-line title in `cisco-cloud-security` is set on a 49 px line pitch instead of the
41 px the master asks for, and the whole block also starts 14 px lower than the reference. The
second line therefore lands outside the 77 px-tall title placeholder and collides with whatever
is painted next: the grey card panel swallows "interface" on slide 18 (evidence-1.png), Group
237's panel shears "your environment" on slide 08 (evidence-2.png), and the diagram panel
leaves a sliver of "Access Security" on slide 11 (evidence-3.png). Slide 04 is the same failure
against the person-node graphic. Single-line titles on the same
master are only 2 px low and read fine (evidence-4.png), which is why the defect is invisible
until a title wraps.

Measured on the raw 960x540 renders, the numbers are the same on every affected slide: the
reference puts line 1's x-height top at row 43 and line 2's at row 84 (pitch 41 px); the
candidate puts them at rows 57 and 106 (pitch 49 px). 49 px is exactly Liberation Sans'
`ascent + descent + lineGap` at 32 pt (1.1499 em x 42.67 px = 49.06 px), i.e. plain 100 %
spacing; 41 px is 0.96 em, i.e. 80 % of 1.2 em.

The cluster lists 7 findings, but the master is the deck's only one, so the defect fires on
every wrapped title: slides 02, 04, 06, 08, 09, 11, 12, 13, 15, 16, 18, 19 and 20 all show the
row-57/row-106 signature. Every deck in the harness carries at least one sub-100 % `lnSpc`
somewhere (90 % is near-universal; `project17` also uses 80 % and 105 %), so the fix is
deck-wide, not deck-specific.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/18 | `Title 1` (id 2): reference line pitch 41 px, candidate 49 px; "interface" drops behind the grey panel that starts at y=1193799 EMU |
| 2 | cisco-cloud-security/08 | `Title 10` (id 11): identical row offsets (43/84 vs 57/106); only the top slivers of "your environment" survive Group 237 |
| 3 | cisco-cloud-security/11 | `Title 1` (id 2): "Access Security" all but disappears under `Rectangle 94`, which follows it in z-order |
| 4 | cisco-cloud-security/03 | the same master, one-line title: candidate cap top row 58 vs reference 56 — anchoring itself works, the block just has to overflow before the bug shows |

## Root cause (hypothesis)

Two independent defects stack. Both are confirmed against the XML and against the pixels.

**1. `<a:lnSpc>` is never parsed (confirmed, dominant).**

`grep -rn "spcPct\|spcPts\|spcBef\|spcAft" crates/ packages/` excluding `docx` returns nothing.
`parse_paragraph_properties` (`crates/pptx-parse/src/drawing.rs:849-881`) reads `algn`, `lvl`,
`marL`, `indent`, the three bullet forms and `defRPr` — and nothing else;
`ParagraphProperties` (`crates/pptx-parse/src/model.rs:303-312`) has no field for line spacing,
space-before or space-after. The value is *never parsed*, not parsed-and-dropped.

The path it would travel is otherwise intact and reachable, which is what makes this a plumbing
job rather than a design one: `parse_text_styles` / `parse_style_levels`
(`crates/pptx-parse/src/drawing.rs:67-98`) already builds the master's nine `titleStyle` levels
through the same `parse_paragraph_properties`, `master_style`
(`crates/pptx-render/src/layout.rs:1786-1801`) already picks `titleStyle` for a `title`
placeholder, and `BodyCascade::paragraph_properties`
(`crates/pptx-render/src/layout.rs:808-828`) already seeds from it before merging master →
layout → slide bodies field-wise (`crates/pptx-render/src/layout.rs:1804-1822`). Confirmed on
the XML: `decks/cisco-cloud-security/xml/18/master.xml`'s `p:titleStyle/a:lvl1pPr` carries
`<a:lnSpc><a:spcPct val="80000"/></a:lnSpc>` with `sz="3200"`; the layout's `Title 1` has only
an `<a:xfrm>` in `spPr` and no `lstStyle`, and the slide's `Title 1` has `<p:spPr/>` plus an
empty `<a:bodyPr/>` and `<a:lstStyle/>` — so `titleStyle` is the only source and the cascade
does reach it.

It dies on the render side even if it were parsed: `ResolvedParagraph`
(`crates/pptx-render/src/layout.rs:910-915`) carries exactly one geometry field,
`margin_left_px`, populated at `crates/pptx-render/src/layout.rs:994-1004`, and
`layout_paragraph` advances by the raw font line box —
`line_y += line_box.height()` (`crates/pptx-render/src/layout.rs:1261`), where `line_box` comes
from `clusters_line_box` → `style_line_box` → `single_line_box`
(`crates/pptx-render/src/layout.rs:1525-1562`) with no spacing rule applied. `ooxml-text`
already has the machinery — `LineSpacingRule` and `apply_spacing_rule`
(`crates/ooxml-text/src/word_metrics.rs:121-128`, `:259`) — but only `docx` calls it
(`crates/ooxml-text/src/measure/line_filler.rs:940`).

The reference's 41 px pitch is `0.8 x 1.2 em`, not `0.8 x` the font's own 1.1499 em line
height (which would be 39.25 px). **Hypothesis, calibrated from the pixels but not from the
spec text:** PowerPoint's `spcPct` is a percentage of a fixed `1.2 x font size`, not of the
font's metric line height, and LibreOffice reproduces that. The measurement supports it to
within a pixel on all seven slides; a fixture test should pin the constant rather than trusting
this note. Within the 41 px box the reference splits ascent/descent in the font's own ratio
(measured ascent 32.3 px, 0.788 of the box; Liberation Sans' `ascent / (ascent+descent+gap)` is
0.787), i.e. exactly the sub-single branch of `apply_spacing_rule`'s `Auto` arm.

**2. The vertical anchor is clamped to zero when the text overflows (confirmed, secondary).**

`crates/pptx-render/src/layout.rs:710-714` computes
`TextAnchor::Center => ((content_rect.h - laid_out.total_height) / 2.0).max(0.0)`. The master's
title `bodyPr` is `anchor="ctr"` with `tIns="45712"`/`bIns="45712"`, and its box is
`cy="731837"` EMU, so `content_rect.h` is 67.2 px. A two-line title is 98 px tall today (82 px
even once `lnSpc` is fixed), so the shift clamps to 0 and the block is effectively top-anchored
at `content_rect.y` = 40.6 px. PowerPoint and LibreOffice centre the block regardless and let it
spill symmetrically.

The arithmetic closes exactly, which is why confidence is high:

| | reference | candidate |
|---|---|---|
| block top | 40.6 + (67.2-82)/2 = **33.2** | 40.6 + max(0, (67.2-98)/2) = **40.6** |
| line-1 ascent inside its box | 41 x 0.788 = **32.3** | full 38.6 |
| predicted baseline 1 | **65.5** | **79.2** |
| measured baseline 1 (x-top + 22.54) | **65.5** | **79.5** |
| predicted line-2 descender bottom | **115.2** | 138.6 |
| measured (slide 08) | **115** | clipped |

So of the 14 px that line 1 sits low, ~7.4 px is the clamp and ~6.3 px is the un-reduced
first-line ascent. Fixing only defect 1 leaves line 2's baseline at 120 px, still 8 px past the
box bottom at 112.6 px and still under the panels — both halves are needed to close the gap.
Evidence-4 is the control: with a one-line title the block fits, the clamp is inert, and the
candidate lands 2 px from the reference.

Not confirmed / out of scope: `compatLnSpc="1"` is set on this master's title `bodyPr` and its
effect on the percentage base was not isolated — every affected shape here carries it, so the
measurements cannot separate it from the plain `spcPct` rule. `spcBef`/`spcAft` share the same
unparsed gap (both appear as `spcPts` across most harness decks, and `project20/04` is already
noted as losing its paragraph gaps in `text-bullets-char-indent-dropped`); they are adjacent,
not part of this cluster's symptom.

## Verification

Re-render `cisco-cloud-security` slides 4, 6, 8, 11, 15, 16, 18 with
`render-improvement-harness/scripts/render_bo.py` and re-run `diff.py`. Two-line titles should
land with line-1 x-height top at row 43 +/-1 and line-2 at row 84 +/-1, matching `lo-img/NN.png`,
and no title glyph should extend past row 116. Current `diff_pct`: 04 9.66, 06 10.22, 08 16.02,
11 10.28, 15 26.91, 16 15.44, 18 21.19 — the title band is a small share of each, so expect
1-3 points off 06/08/18 and less elsewhere; the qualitative check (title fully legible above the
panel) matters more than the number. Slides 02, 09, 12, 13, 19 and 20 have the same signature
without a cluster finding and should improve too. Slide 03 is the regression guard: its
one-line title must not move by more than a pixel.

Re-render at least one slide from every other deck as well — all twelve carry a sub-100 %
`lnSpc` somewhere — and diff against the current output to see what moved.

No existing test covers line spacing or vertical anchoring in `pptx-render`: the module at
`crates/pptx-render/src/layout.rs:2008` has cases for hit testing, master-shape geometry,
reflow, `normAutofit` scaling (`:2239`) and charts, none touching `lnSpc` or `anchor`. New unit
tests belong there. `crates/ooxml-text/tests/docx_text.rs:681-780` already covers
`apply_spacing_rule` itself and should not need changing.
