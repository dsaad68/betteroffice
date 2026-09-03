---
id: text-run-props-misc-property-ignored
title: Apply the run's baseline superscript shift (the italic half is already covered by text-run-props-bold-ignored)
category: text-run-props
impact: low
effort: medium
confidence: high
status: open
occurrences: 2
decks: [cisco-cloud-security, ocp-psp-plan]
findings: [cisco-cloud-security/05/4, ocp-psp-plan/02/3]
files: [crates/pptx-parse/src/model.rs, crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/write.rs, crates/pptx-edit/src/model.rs, crates/pptx-edit/src/story.rs, crates/pptx-edit/src/save.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/display_list.rs, packages/pptx/src/render/canvas.ts]
---

## Symptom

Two unrelated run properties are lost, and they have two different causes.

`baseline="30000"` — the superscript flag — does nothing. The footnote marker after
"75% of mobile apps fail basic security tests" is drawn at the full 12pt run size, sitting on the
same baseline as the sentence it annotates, so the sentence reads "…security tests1"
(evidence-1.png). The reference draws it small and raised.

`i="1"` also does nothing on the four Incentives-pod role subtitles, which render upright
(evidence-2.png). That half is **not a missing property**: `i` is parsed and it reaches font
selection intact. It is the same font-fallback defect already written up as
`text-run-props-bold-ignored`, and fixing that issue fixes these four runs with no extra work.
Only the superscript half needs new code.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/05 | `TextBox 8` footnote marker, `sz="1200" baseline="30000"`: raised and reduced in the reference, full-size on the baseline in the candidate |
| 2 | ocp-psp-plan/02 | The `i="1"` role subtitles under two Incentives-pod names render upright; the bold name above them is likewise unbolded, which is the tell that both come from the same fallback face |

Reading evidence-2.png: the reference's italic is a *serif* italic, because LibreOffice
substitutes an unregistered `Segoe UI` italic with a serif face of its own. The claim here is the
missing slant, not the exact face.

## Root cause (confirmed)

### Superscript: `baseline` is never parsed

`parse_run_properties` (`crates/pptx-parse/src/drawing.rs:900`) reads `sz`, `b`, `i`, `u`,
`a:latin/@typeface`, `a:solidFill`, `lang` and `a:hlinkClick/@r:id`, and nothing else.
`RunProperties` (`crates/pptx-parse/src/model.rs:338`) has no field for a baseline shift, so the
attribute is dropped at the XML boundary and cannot reach layout. Downstream there is nothing to
receive it either: `ResolvedStyle` (`crates/pptx-render/src/layout.rs:924`) carries only face,
family, size, bold, italic, underline and colour, and `PositionedTextRun`
(`crates/pptx-render/src/display_list.rs:230`) is the same list plus geometry.

Confirmed against the deck. The run is
`<a:rPr lang="en-US" sz="1200" baseline="30000" dirty="0">` in
`render-improvement-harness/decks/cisco-cloud-security/xml/05/slide.xml`, and the display list
for that slide merges the marker into its neighbour as one run:

```
{'text': '75% of mobile apps fail basic security tests1', 'fontSizePx': 16.0, ...}
  lineBaseline=452.89  glyphY=[452.89, 452.89, 452.89]
```

The marker has the same `fontSizePx` and the same glyph `y` as the sentence — no size change, no
shift, and not even a run boundary, because `positioned_runs`
(`crates/pptx-render/src/layout.rs:1472`) coalesces adjacent clusters on `(end == start, font_id)`
alone, and a baseline shift is not part of that key.

Measured off the two 96 dpi images (`decks/cisco-cloud-security/{lo,bo}-img/05.png`, red-ink
column runs in the caption band): the base digits ink 12 rows tall in the reference, the marker
inks 6 rows, its baseline sits ~5px above the text baseline. 5px is 31% of the 16px em, which is
exactly `baseline="30000"`. The candidate's marker inks 11 rows on the shared baseline. The raise
therefore comes straight from the attribute; the size reduction to roughly half does not, and is
renderer policy — LibreOffice's superscript default is 58% of the run size.

Not confirmed: what PowerPoint's reduction ratio is. The repo's own docx convention is 0.75 with a
0.4em raise (`crates/ooxml-text/src/measure/prepare.rs:466`), which is larger than what the
reference draws here. Note that pptx-render does not use the `ooxml-text` measure pipeline at all
— it imports only `shape`, `break_opportunities` and `single_line_box`
(`crates/pptx-render/src/layout.rs:8`) — so the docx superscript support is not reusable, only its
convention is.

`baseline="0"` also appears on `defRPr` in the layout's `lstStyle`
(`decks/cisco-cloud-security/xml/05/slide.xml`), and `parse_run_properties` serves `defRPr` too,
so once modeled the cascade picks it up for free and a `0` is a no-op shift.

### Italic: a duplicate of `text-run-props-bold-ignored`

`i="1"` is parsed (`crates/pptx-parse/src/drawing.rs:911`), stored
(`crates/pptx-parse/src/model.rs:341`), carried through the snapshot
(`crates/pptx-edit/src/story.rs:645`, `crates/pptx-edit/src/model.rs:38`), resolved
(`crates/pptx-render/src/layout.rs:1020`) and passed to `resolve_face`
(`crates/pptx-render/src/layout.rs:1041`). It survives all the way to the display list — the run
for "Incentives/Investment " comes out with `italic: True`.

It is thrown away in `resolve_face` (`crates/pptx-render/src/layout.rs:245`):

```rust
self.faces
    .get(&(normalized.clone(), bold, italic))                // 253
    .or_else(|| self.faces.get(&(normalized, false, false)))  // 254
    .or(self.fallback.as_ref())                              // 255
```

These runs ask for `Segoe UI`, which no host registers, so both lookups miss and the run lands on
`self.fallback` — the first face ever registered (`crates/pptx-render/src/layout.rs:111`), which
carries no style. Nothing downstream can recover: there is no synthetic oblique anywhere in
`crates/pptx-raster/src/font.rs`, which only fills the outlines of the face it is handed.

Confirmed by experiment. Rendering ocp-psp-plan slide 2 twice through `bindings/python-pptx`,
changing nothing but which Liberation Sans face is registered first, the italic run resolves to
`fontId 0` both times — that is, to whichever face happened to be registered first:

```
regular-first face ids per style: {'Regular': 2, 'Bold': 5, 'Italic': 8, 'BoldItalic': 11}
  -> italic run resolves to fontId 0   (= the Arial Regular registered first)
italic-first  face ids per style: {'Italic': 2, 'Regular': 5, 'Bold': 8, 'BoldItalic': 11}
  -> italic run resolves to fontId 0   (= the Arial Italic registered first)
```

This is the defect `text-run-props-bold-ignored` describes, and its proposed fix — a
style-aware fallback chain that degrades family before style — resolves `(fallback family,
false, true)` and gives these runs Liberation Sans Italic. No separate work is needed.

The browser backend does not share the defect for either flag:
`packages/pptx/src/render/canvas.ts:233` builds its CSS font from `run.italic`, so the canvas
already slants text the raster leaves upright. Fixing `resolve_face` removes that divergence too.

## Verification

Superscript: re-render and re-diff cisco-cloud-security.

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py cisco-cloud-security
.venv/bin/python render-improvement-harness/scripts/diff.py cisco-cloud-security
```

Slide 05's `fine_pct` is 4.52 and is dominated by the `custGeom` icons
(`geometry-custom-collapses-to-bbox`), so expect only a fraction of a percent from this change.
The real check is the ink: in `bo-img/05.png` the caption band's last red column run must ink
about 6 rows ending ~5px above the sentence baseline, matching `lo-img/05.png`, instead of the 11
rows on the shared baseline it inks today.

Italic: nothing to verify separately — it is covered by `text-run-props-bold-ignored`'s
verification on ocp-psp-plan/02.

No existing coverage for either. `crates/pptx-parse/src/drawing.rs:956`
(`parses_text_formatting_and_nested_shape_types`) is the `rPr` fixture a `baseline` case belongs in; the `crates/pptx-render/src/layout.rs`
test module (`crates/pptx-render/src/layout.rs:2008`) has nothing asserting run size or glyph `y`
for a shifted run; `crates/pptx-raster/tests/golden.rs` has no superscript fixture.
