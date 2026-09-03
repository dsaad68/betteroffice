---
id: text-font-substitution-issues
title: Resolve "+mj-lt" to the theme's major font
category: text-font
impact: medium
effort: medium
confidence: high
status: open
occurrences: 8
decks: [project17]
findings: [project17/02/1, project17/02/2, project17/05/2, project17/10/3, project17/11/2, project17/12/2, project17/12/3, project17/12/4]
files: [crates/ooxml-drawingml/src/theme.rs, crates/pptx-render/src/layout.rs, crates/pptx-parse/src/drawing.rs]
---

## Symptom

Every run in project17 that asks for the theme's major latin font (`<a:latin typeface="+mj-lt"/>`)
is drawn in a face roughly 9-12% wider than the reference's. Lines therefore wrap one word
earlier: slide titles gain a line and drop out of the fixed-height purple title band, where white
glyphs land on white background and become unreadable (evidence-1.png), and every question/answer
box on slide 12 grows a line and pushes its last line onto or past its own border
(evidence-2.png). The same slides also show a Wingdings private-use glyph rendering as a `.notdef`
tofu box (evidence-3.png).

The two symptoms have different causes and are separated below. The width/wrap half is a renderer
bug and is confirmed; the tofu half is a renderer gap whose visible effect the harness cannot
currently fix on its own.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project17/12 | Title: reference fits two lines in the band, candidate wraps at "Middle East" and drops line 2 into the white body |
| 2 | project17/12 | Left-column question boxes: candidate needs one extra line per box, last line sits on/past the border |
| 3 | project17/02 | Timeline bullets: reference draws the Wingdings arrow and the Calibri "ti" ligature, candidate draws tofu and unligated, wider glyphs |
| 4 | project17/12 | Same build, three ways: reference, candidate today, candidate with Carlito additionally registered as `Calibri Light` - the wrap point snaps back to the reference's |

## Root cause (confirmed)

### 1. `+mj-lt` resolves to the *minor* font (findings 02/1, 05/2, 10/3, 11/2, 12/2, 12/4)

project17's theme is unusual: it puts the regular face in `majorFont` and the light face in
`minorFont`.

```xml
<a:fontScheme>
  <a:majorFont><a:latin typeface="Calibri"/></a:majorFont>
  <a:minorFont><a:latin typeface="Calibri Light"/></a:minorFont>
</a:fontScheme>
```

The theme is parsed correctly - `majorFont` lands in `font_scheme.major_font`
(`crates/pptx-parse/src/theme.rs:60`). The defect is in the resolver.
`crates/pptx-render/src/layout.rs:1034` routes any family starting with `+` through
`resolve_theme_font_ref` (`crates/ooxml-drawingml/src/theme.rs:191`), whose major/minor decision is

```rust
if lower.contains("major") {          // theme.rs:203
    get_major_font(theme, script)
} else {
    get_minor_font(theme, script)
}
```

`resolve_theme_font_ref` was written for WordprocessingML, whose references spell out
`majorAscii` / `minorHAnsi` (its only tests use those forms -
`crates/docx-parse/src/theme.rs:244`). DrawingML spells the same thing `+mj-lt` / `+mn-lt`, which
does not contain the substring `major`, so **every** `+mj-*` reference in a pptx falls through to
`get_minor_font` (`crates/ooxml-drawingml/src/theme.rs:166`).

For project17 that turns `+mj-lt` into `Calibri Light`. The harness registers Carlito under the
names `Calibri` and `Carlito` only (`render-improvement-harness/scripts/render_bo.py:15`), so
`Calibri Light` misses both lookups in `resolve_face`
(`crates/pptx-render/src/layout.rs:245-257`) - `normalize_family` is a lowercase-and-trim with no
aliasing (`crates/pptx-render/src/layout.rs:1978`) - and the run lands on `self.fallback`, the
first face ever registered (`crates/pptx-render/src/layout.rs:111`), which for this harness is
Liberation Sans Regular.

That is exactly the observed width delta. Measured with the shipped faces at 100px:

| string | Carlito | Liberation Sans | delta |
|---|---|---|---|
| the slide-12 title's first line | 3199.5 px | 3498.3 px | +9.3% |
| `Agenda` (slide 02 title) | 306.9 px | 344.8 px | +12.4% |

which matches the +8.5% / +12.5% the slide reports measured off the pixels.

Confirmed by experiment rather than by reading alone. Registering Carlito under the *extra* name
`Calibri Light` - i.e. giving the wrongly-resolved family the right metrics, without touching the
crates - restores the reference's wrap points (evidence-4.png) and drops the diff on every slide
in the cluster:

| slide | fine_pct today | fine_pct with `Calibri Light` registered |
|---|---|---|
| project17/02 | 3.10 | 2.33 |
| project17/05 | 15.24 | 12.15 |
| project17/10 | 11.10 | 10.36 |
| project17/11 | 18.94 | 18.53 |
| project17/12 | 11.15 | 7.72 |

`+mj-lt` occurs 684 times across the corpus, 461 of them in project17
(`render-improvement-harness/decks/*/xml`).

### 2. `<a:sym>` is never parsed (finding 02/2)

`parse_run_properties` reads a typeface from `<a:latin>` and nothing else
(`crates/pptx-parse/src/drawing.rs:913-916`); there is no `ea`, `cs` or `sym` on
`RunProperties`. `"sym"` appears in the crates only as an element name the writer preserves in
document order (`crates/pptx-parse/src/write.rs:1511`).

The slide-02 bullet run is

```xml
<a:rPr sz="1400" ...><a:latin typeface="Sakkal Majalla"/>
  <a:sym typeface="Wingdings" charset="2"/></a:rPr>
<a:t>&#xF0E8;</a:t>
```

so the renderer shapes U+F0E8 with the *latin* face, gets glyph 0, and paints the `.notdef` box.
There is also no glyph-level fallback on the pptx path: `shape` takes a single `FontId`
(`crates/ooxml-text/src/shape.rs:52`) and `crates/pptx-render/src/layout.rs:1041` hands it one
resolved face, so the chain resolver that already exists
(`crates/ooxml-text/src/font_store.rs:398`) is never used here. docx has per-run fallback chains
(`crates/docx-layout/src/display_list.rs:1154`); pptx does not.

**Harness limitation, separated:** even with `<a:sym>` parsed, this bullet would still be tofu in
the harness. `packages/fonts/assets` ships only Liberation/Carlito/Caladea/Noto - no Wingdings or
OpenSymbol-class face - so there is nothing for `Wingdings` to resolve to. LibreOffice draws the
arrow because it substitutes OpenSymbol and applies the Wingdings PUA mapping. The parse gap is a
real renderer defect; the tofu in evidence-3.png cannot be cleared by a code change alone.

### Not confirmed

- **Finding 12/3** (the "Body copy..." bar's second line touching its bottom edge) is
  *not* caused by font substitution. With correct metrics the bar's text still wraps at the same
  point and still crowds the bottom edge (evidence-4.png, bottom panel). The candidate places the
  whole title/bar text lower than the reference does, which looks like a vertical
  anchoring / `lnSpc spcPct 80000` issue and belongs with `text-layout-master-lnspc-ignored` or
  `text-overflow-autofit-not-handled`, not here.
- **Finding 12/2**'s second symptom - the overflowed line rendering white-on-white - is a
  consequence of the wrap, but the residual vertical offset above means fixing the font alone does
  not put line 2 back inside the band on slide 12. Expect the wrap to match and the remaining gap
  to be the vertical one.

### Related defect in the same function, not required here

`resolve_theme_font_ref`'s script detection is also DOCX-shaped: it tests for `eastasia` /
`bidi` (`crates/ooxml-drawingml/src/theme.rs:196-199`), so `+mn-ea` and `+mj-ea` resolve as
*latin*. `+mn-cs` happens to work only because `"+mn-cs"` contains the substring `cs`. The corpus
has 5379 `+mn-ea` references, but every theme in it leaves `<a:ea typeface=""/>` empty, so
correcting the script today would resolve them to an empty family. Worth fixing together with the
major/minor test, with the empty-string case falling back to latin.

## Verification

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py project17
.venv/bin/python render-improvement-harness/scripts/diff.py project17
```

Expect project17/12 to fall from 11.15 to roughly 8, 05 from 15.24 to roughly 12, 02 from 3.10 to
roughly 2.3, 10 and 11 to improve slightly. Nothing should regress in project17.

**Watch the other decks.** project17 is the only deck in the corpus whose font scheme is inverted;
every other deck has the conventional `major = "<X> Light"`, `minor = "<X>"`:

| deck | majorFont | minorFont | today `+mj-lt` gives | after the fix |
|---|---|---|---|---|
| project17 | Calibri | Calibri Light | Calibri Light (unregistered) | Calibri -> Carlito |
| ocp-psp-plan, flat-chart, green-solutions, minimal-chart, stacked-bar, swot-analysis, triangles-corporate, typography-trick | Calibri Light | Calibri | Calibri -> Carlito | Calibri Light (unregistered) |
| project20, rollout-plan, ocp-psp-plan (10 masters) | Segoe UI Light | Segoe UI | Segoe UI (unregistered) | Segoe UI Light (unregistered) |
| cisco-cloud-security | Arial | Arial | Arial | Arial |

So the bug is currently *masking* itself on the Calibri-Light decks, and correcting `+mj-lt`
alone would regress them from Carlito to the bare fallback. Re-diff `ocp-psp-plan`,
`typography-trick` and the chart decks alongside project17, and pair the fix with the family
substitution described in `possible-solution.md`. LibreOffice maps `Calibri Light` to Carlito, so
matching it needs that substitution, not just the correct theme slot.

Existing coverage: none. `crates/ooxml-drawingml/src/theme.rs:247` only asserts the Office
defaults through `get_major_font`/`get_minor_font` directly, never through
`resolve_theme_font_ref`; the only `resolve_theme_font_ref` tests are the DOCX-form ones at
`crates/docx-parse/src/theme.rs:244-247`. `crates/pptx-render` never mentions `+mj-lt` outside
`crates/pptx-render/src/layout.rs:1040`, which passes `+mn-lt`.
