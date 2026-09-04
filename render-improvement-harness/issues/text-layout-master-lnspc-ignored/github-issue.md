# pptx: Master title style's reduced line spacing (lnSpc) not applied

**Describe the bug**

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

Seen on 7 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/18** `Title 1` (id 2): reference line pitch 41 px, candidate 49 px; "interface" drops behind the grey panel that starts at y=1193799 EMU

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-layout-master-lnspc-ignored/evidence-1.png)

**2. cisco-cloud-security/08** `Title 10` (id 11): identical row offsets (43/84 vs 57/106); only the top slivers of "your environment" survive Group 237

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-layout-master-lnspc-ignored/evidence-2.png)

**3. cisco-cloud-security/11** `Title 1` (id 2): "Access Security" all but disappears under `Rectangle 94`, which follows it in z-order

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-layout-master-lnspc-ignored/evidence-3.png)

**4. cisco-cloud-security/03** the same master, one-line title: candidate cap top row 58 vs reference 56 — anchoring itself works, the block just has to overflow before the bug shows

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-layout-master-lnspc-ignored/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 4, 6, 8, 11, 15, 16, 18

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

Two independent defects stack. Both are confirmed against the XML and against the pixels.

**1. `<a:lnSpc>` is never parsed (confirmed, dominant).**

`grep -rn "spcPct\|spcPts\|spcBef\|spcAft" crates/ packages/` excluding `docx` returns nothing.
`parse_paragraph_properties` ([`crates/pptx-parse/src/drawing.rs:849-881`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L849-L881)) reads `algn`, `lvl`,
`marL`, `indent`, the three bullet forms and `defRPr` — and nothing else;
`ParagraphProperties` ([`crates/pptx-parse/src/model.rs:303-312`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L303-L312)) has no field for line spacing,
space-before or space-after. The value is *never parsed*, not parsed-and-dropped.

The path it would travel is otherwise intact and reachable, which is what makes this a plumbing
job rather than a design one: `parse_text_styles` / `parse_style_levels`
([`crates/pptx-parse/src/drawing.rs:67-98`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L67-L98)) already builds the master's nine `titleStyle` levels
through the same `parse_paragraph_properties`, `master_style`
([`crates/pptx-render/src/layout.rs:1786-1801`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1786-L1801)) already picks `titleStyle` for a `title`
placeholder, and `BodyCascade::paragraph_properties`
([`crates/pptx-render/src/layout.rs:808-828`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L808-L828)) already seeds from it before merging master →
layout → slide bodies field-wise ([`crates/pptx-render/src/layout.rs:1804-1822`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1804-L1822)). Confirmed on
the XML: `decks/cisco-cloud-security/xml/18/master.xml`'s `p:titleStyle/a:lvl1pPr` carries
`<a:lnSpc><a:spcPct val="80000"/></a:lnSpc>` with `sz="3200"`; the layout's `Title 1` has only
an `<a:xfrm>` in `spPr` and no `lstStyle`, and the slide's `Title 1` has `<p:spPr/>` plus an
empty `<a:bodyPr/>` and `<a:lstStyle/>` — so `titleStyle` is the only source and the cascade
does reach it.

It dies on the render side even if it were parsed: `ResolvedParagraph`
([`crates/pptx-render/src/layout.rs:910-915`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L910-L915)) carries exactly one geometry field,
`margin_left_px`, populated at [`crates/pptx-render/src/layout.rs:994-1004`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L994-L1004), and
`layout_paragraph` advances by the raw font line box —
`line_y += line_box.height()` ([`crates/pptx-render/src/layout.rs:1261`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1261)), where `line_box` comes
from `clusters_line_box` → `style_line_box` → `single_line_box`
([`crates/pptx-render/src/layout.rs:1525-1562`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1525-L1562)) with no spacing rule applied. `ooxml-text`
already has the machinery — `LineSpacingRule` and `apply_spacing_rule`
([`crates/ooxml-text/src/word_metrics.rs:121-128`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/word_metrics.rs#L121-L128), `:259`) — but only `docx` calls it
([`crates/ooxml-text/src/measure/line_filler.rs:940`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/measure/line_filler.rs#L940)).

The reference's 41 px pitch is `0.8 x 1.2 em`, not `0.8 x` the font's own 1.1499 em line
height (which would be 39.25 px). **Hypothesis, calibrated from the pixels but not from the
spec text:** PowerPoint's `spcPct` is a percentage of a fixed `1.2 x font size`, not of the
font's metric line height, and LibreOffice reproduces that. The measurement supports it to
within a pixel on all seven slides; a fixture test should pin the constant rather than trusting
this note. Within the 41 px box the reference splits ascent/descent in the font's own ratio
(measured ascent 32.3 px, 0.788 of the box; Liberation Sans' `ascent / (ascent+descent+gap)` is
0.787), i.e. exactly the sub-single branch of `apply_spacing_rule`'s `Auto` arm.

**2. The vertical anchor is clamped to zero when the text overflows (confirmed, secondary).**

[`crates/pptx-render/src/layout.rs:710-714`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L710-L714) computes
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

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Four changes, in dependency order. The first three are the `lnSpc` plumb; the fourth is the
anchor clamp, which has to land with them or the two-line titles still overflow.

**Model the spacing.** Add `line_spacing: Option<LineSpacing>` to `ParagraphProperties`
([`crates/pptx-parse/src/model.rs:303-312`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L303-L312)) with
`enum LineSpacing { Percent(f64), Points(f64) }`, mirroring how `TextAutofit`
([`crates/pptx-parse/src/model.rs:286-292`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L286-L292)) is shaped. `ParagraphProperties` is
`Serialize`/`Deserialize` and only re-exported by [`crates/betteroffice-pptx/src/types.rs:9`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/betteroffice-pptx/src/types.rs#L9), so
the field is additive — nothing in `pptx-edit` or `pptx-parse/src/write.rs` reads it.

**Parse it.** In `parse_paragraph_properties`
([`crates/pptx-parse/src/drawing.rs:849-881`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L849-L881)), read `lnSpc`'s single child: `spcPct val` through
the existing `percentage_attribute` helper ([`crates/pptx-parse/src/drawing.rs:806-813`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L806-L813)), `spcPts
val` as hundredths of a point. This is the only edit needed on the parse side — `titleStyle`,
`bodyStyle`, `otherStyle`, layout `lstStyle` and slide `pPr` all funnel through this one
function via `parse_style_levels` ([`crates/pptx-parse/src/drawing.rs:78`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L78)). Add `space_before` /
`space_after` here too if the adjacent gap is in scope; the cascade below carries them for free.

**Cascade and apply it.** Extend `merge_paragraph_properties`
([`crates/pptx-render/src/layout.rs:1804-1822`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1804-L1822)) with the same `is_some()` override the other five
fields use, add `line_spacing: Option<LineSpacing>` to `ResolvedParagraph`
([`crates/pptx-render/src/layout.rs:910-915`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L910-L915)), populate it at
[`crates/pptx-render/src/layout.rs:994-1004`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L994-L1004) next to `margin_left_px`, and consume it in
`layout_paragraph`: run every `clusters_line_box` / `style_line_box` result
([`crates/pptx-render/src/layout.rs:1525-1562`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1525-L1562)) through a `spaced_line_box` helper before the
line's `height`, `baseline` and the `line_y +=` advance at
[`crates/pptx-render/src/layout.rs:1261`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1261) are taken from it. The helper does what the measurements
say LibreOffice does:

- `Percent(p)`: target height = `p * 1.2 * font_size_px` — the 1.2 constant is PowerPoint's, not
  the font's 1.1499 em, and is what reproduces the reference's 41 px at 32 pt / 80 %.
- `Points(pt)`: target height = `points_to_px(pt)`.
- Then redistribute: if `target < ascent + descent`, scale both by `target / (ascent + descent)`
  and zero the leading; otherwise keep ascent and descent and put the slack in `leading`. That is
  literally the `Auto` arm of `apply_spacing_rule`
  ([`crates/ooxml-text/src/word_metrics.rs:260-281`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/word_metrics.rs#L260-L281)), so express it as
  `apply_spacing_rule(content, &LineSpacingRule::Auto { line_240ths: (240.0 * target / content.height()).round() as u32 })`
  rather than reimplementing the redistribution, or add a `Scaled { target_px }` arm to
  `LineSpacingRule` if the 240ths round-trip loses too much precision at small sizes.

Everything downstream already carries per-line geometry: `PositionedTextLine`
([`crates/pptx-render/src/display_list.rs:209-219`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L209-L219)) has explicit `height` and `baseline`, and
[`packages/pptx/src/render/canvas.ts:224`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L224) paints from `line.baseline`, so the display-list
contract does not change.

**Unclamp the anchor.** Drop the `.max(0.0)` from the `Center` and `Bottom` arms at
[`crates/pptx-render/src/layout.rs:710-714`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L710-L714) so an overflowing block centres about the box instead
of pinning to the top inset. `overflow` ([`crates/pptx-render/src/layout.rs:739`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L739)) still reports
the spill, and `shift_line` ([`crates/pptx-render/src/layout.rs:1565`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1565)) already handles a negative
`y`.

```rust
// pptx-parse/src/drawing.rs, in parse_paragraph_properties
line_spacing: element.child("lnSpc").and_then(|spacing| {
    if let Some(pct) = spacing.child("spcPct") {
        percentage_attribute(pct, "val").map(LineSpacing::Percent)
    } else {
        spacing
            .child("spcPts")
            .and_then(|pts| numeric_attribute(Some(pts), "val"))
            .map(|value| LineSpacing::Points(value as f64 / 100.0))
    }
}),

// pptx-render/src/layout.rs, in layout_paragraph, replacing the bare line box
let content = clusters_line_box(fonts, slice, scale)?;
let line_box = spaced_line_box(content, paragraph.line_spacing, primary_size_px(slice, scale));
// ... height: line_box.height(), baseline: line_y + line_box.ascent, line_y += line_box.height()

fn spaced_line_box(content: LineBox, spacing: Option<LineSpacing>, size_px: f32) -> LineBox {
    let target = match spacing {
        Some(LineSpacing::Percent(p)) => p as f32 * PPT_PERCENT_LINE_EM * size_px, // 1.2
        Some(LineSpacing::Points(pt)) => points_to_px(pt as f32),
        None => return content,
    };
    if content.height() <= 0.0 { return content; }
    apply_spacing_rule(content, &LineSpacingRule::Auto {
        line_240ths: ((240.0 * target / content.height()).round() as u32).max(1),
    })
}

// pptx-render/src/layout.rs:710
TextAnchor::Center => (content_rect.h - laid_out.total_height) / 2.0,
TextAnchor::Bottom => content_rect.h - laid_out.total_height,
```

Risks and tests to add:

- **This moves text on every deck.** All twelve harness decks carry a sub-100 % `lnSpc`
  somewhere (90 % almost everywhere, plus 80 % and 105 % in `project17`), and the anchor unclamp
  touches every overflowing `anchor="ctr"`/`"b"` shape whether or not it has an `lnSpc`. Re-run
  the whole harness, not just `cisco-cloud-security`, and treat unrelated `diff_pct` regressions
  as blocking.
- **The 1.2 constant is calibrated, not sourced.** It reproduces the reference to within a pixel
  on seven slides at one size (32 pt) in one font, and the alternative reading (percentage of the
  font's own line height) is off by 1.8 px there. Add a fixture at a second size and a second
  font before trusting it; if the two readings disagree on those, the constant is wrong.
- **`normAutofit lnSpcReduction` becomes reachable.** It is already parsed into
  `TextAutofit::Normal` ([`crates/pptx-parse/src/model.rs:286-292`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L286-L292),
  [`crates/pptx-parse/src/drawing.rs:802`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L802)) and, like `fontScale`
  ([`crates/pptx-render/src/layout.rs:681-685`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L681-L685)), ignored. Once a line-height knob exists it is a
  two-line follow-up, but it also interacts with the autofit shrink loop at
  [`crates/pptx-render/src/layout.rs:687-698`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L687-L698), which re-lays out at a smaller `scale` — the
  spacing must scale with the font size, not be recomputed from an unscaled one. Feed
  `style.font_size_pt * scale` into `spaced_line_box`, the same value `style_line_box`
  ([`crates/pptx-render/src/layout.rs:1558-1561`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1558-L1561)) already uses.
- **Mixed-size lines** need a rule for which run's size drives `Percent`. PowerPoint uses the
  largest run on the line; `clusters_line_box` ([`crates/pptx-render/src/layout.rs:1525-1548`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1525-L1548))
  already takes the max ascent/descent/leading, so take the max font size over the same
  deduplicated run set.
- **Hit testing and caret geometry** read `line.y` / `line.height` / `caret_stops`
  ([`crates/pptx-render/src/layout.rs:296`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L296)), so tighter lines change which line a click lands on
  in `packages/pptx-react`. Nothing needs a code change, but
  `packages/pptx-react/src/interactions.test.ts` hard-codes baselines (`:63`, `:77`) and will
  need its fixtures refreshed if they come from a real deck.
- Tests to add in `crates/pptx-render/src/layout.rs`'s module (`:2008`): a title placeholder
  inheriting `titleStyle` `lnSpc 80%` produces a two-line block whose line pitch is
  `0.8 * 1.2 * font_size_px`; a slide-level `pPr/lnSpc` overrides the master's; `spcPts` gives a
  fixed pitch independent of font size; an `anchor="ctr"` box whose text overflows places the
  block symmetrically (negative shift) while a fitting one is unchanged.

**How to verify**

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
[`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2008) has cases for hit testing, master-shape geometry,
reflow, `normAutofit` scaling (`:2239`) and charts, none touching `lnSpc` or `anchor`. New unit
tests belong there. [`crates/ooxml-text/tests/docx_text.rs:681-780`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/tests/docx_text.rs#L681-L780) already covers
`apply_spacing_rule` itself and should not need changing.

**Additional context**

none.

Related issues found in the same run: `text-bullets-char-indent-dropped`

Files most likely involved: `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/drawing.rs`, `crates/pptx-render/src/layout.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-layout-master-lnspc-ignored/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
