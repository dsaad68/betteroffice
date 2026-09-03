---
id: text-run-props-bold-ignored
title: Keep the requested weight when a run's font family is not registered
category: text-run-props
impact: high
effort: easy
confidence: high
status: open
occurrences: 15
decks: [ocp-psp-plan, project17, project20, rollout-plan]
findings: [ocp-psp-plan/02/2, ocp-psp-plan/07/2, project17/07/3, project20/03/2, project20/04/4, project20/05/2, project20/07/3, project20/09/3, project20/11/2, project20/12/2, project20/13/2, project20/14/1, project20/16/1, rollout-plan/02/2, rollout-plan/03/2]
files: [crates/pptx-render/src/layout.rs, crates/ooxml-drawingml/src/theme.rs]
---

## Symptom

Runs that carry an explicit `b="1"` are drawn at regular weight. Slide titles, pod and column
headers, and the bold leading letter of an emphasis run all come out thin (evidence-1.png,
evidence-2.png, evidence-4.png) where the reference renders them bold. The failure is not
confined to whole runs: where a paragraph mixes a bold run with a regular one, both render
identically (evidence-3.png, evidence-4.png), so the emphasis disappears entirely.

The common factor across all 15 findings is that the run's resolved latin family is a face the
host never registered — `Segoe UI`, `Segoe UI Semibold`, `Century`, `Calibri Light`. It is not
that `b="1"` is dropped; it is that the substitute face picked for the missing family is always
the regular one.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project20/04 | `Workstream Tracker`, `sz="3600" b="1"` on the theme minor font (`Segoe UI`): bold in the reference, regular in the candidate |
| 2 | ocp-psp-plan/02 | Both pod headers, `sz="1400" b="1"` with an explicit `<a:latin typeface="Segoe UI"/>`, render at regular weight |
| 3 | rollout-plan/03 | The R/A/C/I legend: the reference bolds only the leading letter of each word (its own `b="1"` run), the candidate bolds nothing |
| 4 | ocp-psp-plan/07 | `L100 – ` / `L200 – ` / `SME – ` prefixes are separate `b="1"` runs; the candidate renders the whole line at one weight |

## Root cause (confirmed)

`b="1"` is parsed into `RunProperties::bold` (`crates/pptx-parse/src/drawing.rs:910`,
`crates/pptx-parse/src/model.rs:340`) and copied into the snapshot's `TextStyle::bold`
(`crates/pptx-edit/src/story.rs:645`, `crates/pptx-edit/src/model.rs:37`). It survives the cascade:
`crates/pptx-render/src/layout.rs:1016` resolves the effective `bold` and
`crates/pptx-render/src/layout.rs:1041` passes it to `resolve_face`. The flag is not lost on the
way in.

`SlideRenderer::resolve_face` (`crates/pptx-render/src/layout.rs:245`) is where the weight is
thrown away:

```rust
self.faces
    .get(&(normalized.clone(), bold, italic))                 // 253
    .or_else(|| self.faces.get(&(normalized, false, false)))   // 254
    .or(self.fallback.as_ref())                                // 255
```

`self.fallback` is a single `FontFace` set to whichever face was registered first
(`crates/pptx-render/src/layout.rs:111`), with no memory of its bold/italic key. So when the
requested family is absent from `self.faces` — the case in every finding here — both lookups miss
and every run, bold or not, lands on that one face. Line 254 drops the weight a second time for
the case where the family is registered but only in regular.

`normalize_family` is a lowercase-and-trim (`crates/pptx-render/src/layout.rs:1978`) and there is
no family aliasing anywhere, so `Segoe UI` never reaches a registered face under another name.

The face chosen here is the only thing that decides the rasterized weight: layout shapes with
`run.style.face.id` (`crates/pptx-render/src/layout.rs:1383`) and `crates/pptx-raster/src/font.rs`
only fills the glyph outlines it is handed — there is no synthetic emboldening.

Confirmed by experiment, not by reading alone. Rendering project20 slide 4 twice through
`bindings/python-pptx`, changing nothing but the order of two `register_font` calls, flips the
title's weight:

```python
deck = bo.Presentation.open_path("render-improvement-harness/decks/project20/source.pptx")
# registering the bold face FIRST makes self.fallback bold and the b="1" title renders bold;
# registering the regular face first renders it thin, exactly as in evidence-1.png
deck.register_font("Arial", bold_bytes, bold=True, italic=False)
deck.register_font("Arial", regular_bytes, bold=False, italic=False)
deck.render_png(3, scale=1.0, background="slide")
```

The rendered weight therefore depends on host registration order, not on `b`.

Two side notes, both hypotheses the fix should keep in mind:

- `resolve_theme_font_ref` treats `+mj-lt` as a *minor* font reference: it tests
  `lower.contains("major")` (`crates/ooxml-drawingml/src/theme.rs:203`), which `+mj-lt` does not
  satisfy, so it falls through to `get_minor_font`. project17/07/3 uses
  `<a:latin typeface="+mj-lt"/>` against a theme whose major latin is `Calibri` (registered, with
  a bold face) and whose minor latin is `Calibri Light` (not registered) — so that finding needs
  this bug fixed too, otherwise the run keeps resolving to an unregistered family. It looks like
  a distinct defect worth its own issue; it is not required for the other 14 findings.
- The canvas backend does not share the defect: `packages/pptx/src/render/canvas.ts:234` builds
  its CSS font from the display list's `run.bold`, so the browser draws bold glyphs while layout
  measured advances on the regular face. Fixing `resolve_face` also removes that raster/canvas
  divergence.

Not confirmed: project20/05/2 also reports the title at roughly half the expected glyph height.
That is a size problem on top of the weight problem, is not explained by `resolve_face`, and
belongs with the autofit cluster.

## Verification

Re-render the four decks and re-diff:

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py project20
.venv/bin/python render-improvement-harness/scripts/diff.py project20
```

`render_bo.py` registers Liberation Sans / Carlito / Caladea in all four styles, so a corrected
fallback has a bold face to reach. Expect the biggest drops on the slides whose diff is dominated
by title and header strokes: project20/12 (`fine_pct` 11.66), project20/13 (13.93),
ocp-psp-plan/02 (14.25), project17/07 (13.83, only if the `+mj-lt` note above is addressed too).
project20/05 (52.06) and rollout-plan/03 (44.94) are dominated by other clusters and should
improve only slightly. No slide should regress.

There is no existing coverage: `crates/pptx-render/src/layout.rs:2018` registers `Arial` in both
weights but nothing asserts what an unregistered family resolves to, and
`crates/pptx-raster/tests/golden.rs` only exercises `bold: false`. Unit tests on `resolve_face`
belong in the `layout.rs` test module; a bold golden image under
`crates/pptx-raster/tests/golden` would lock the raster side down.
