# pptx: Explicit run-level bold (b="1") not applied

**Describe the bug**

Runs that carry an explicit `b="1"` are drawn at regular weight. Slide titles, pod and column
headers, and the bold leading letter of an emphasis run all come out thin (evidence-1.png,
evidence-2.png, evidence-4.png) where the reference renders them bold. The failure is not
confined to whole runs: where a paragraph mixes a bold run with a regular one, both render
identically (evidence-3.png, evidence-4.png), so the emphasis disappears entirely.

The common factor across all 15 findings is that the run's resolved latin family is a face the
host never registered — `Segoe UI`, `Segoe UI Semibold`, `Century`, `Calibri Light`. It is not
that `b="1"` is dropped; it is that the substitute face picked for the missing family is always
the regular one.

Seen on 15 slides across 4 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project20/04** `Workstream Tracker`, `sz="3600" b="1"` on the theme minor font (`Segoe UI`): bold in the reference, regular in the candidate

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-bold-ignored/evidence-1.png)

**2. ocp-psp-plan/02** Both pod headers, `sz="1400" b="1"` with an explicit `<a:latin typeface="Segoe UI"/>`, render at regular weight

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-bold-ignored/evidence-2.png)

**3. rollout-plan/03** The R/A/C/I legend: the reference bolds only the leading letter of each word (its own `b="1"` run), the candidate bolds nothing

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-bold-ignored/evidence-3.png)

**4. ocp-psp-plan/07** `L100 – ` / `L200 – ` / `SME – ` prefixes are separate `b="1"` runs; the candidate renders the whole line at one weight

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-bold-ignored/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 7
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 7
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 3, 4, 5, 7, 9, 11, 12, 13, 14, 16
- `rollout-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 3

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

`b="1"` is parsed into `RunProperties::bold` ([`crates/pptx-parse/src/drawing.rs:910`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L910),
[`crates/pptx-parse/src/model.rs:340`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L340)) and copied into the snapshot's `TextStyle::bold`
([`crates/pptx-edit/src/story.rs:645`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L645), [`crates/pptx-edit/src/model.rs:37`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L37)). It survives the cascade:
[`crates/pptx-render/src/layout.rs:1016`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1016) resolves the effective `bold` and
[`crates/pptx-render/src/layout.rs:1041`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1041) passes it to `resolve_face`. The flag is not lost on the
way in.

`SlideRenderer::resolve_face` ([`crates/pptx-render/src/layout.rs:245`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L245)) is where the weight is
thrown away:

```rust
self.faces
    .get(&(normalized.clone(), bold, italic))                 // 253
    .or_else(|| self.faces.get(&(normalized, false, false)))   // 254
    .or(self.fallback.as_ref())                                // 255
```

`self.fallback` is a single `FontFace` set to whichever face was registered first
([`crates/pptx-render/src/layout.rs:111`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L111)), with no memory of its bold/italic key. So when the
requested family is absent from `self.faces` — the case in every finding here — both lookups miss
and every run, bold or not, lands on that one face. Line 254 drops the weight a second time for
the case where the family is registered but only in regular.

`normalize_family` is a lowercase-and-trim ([`crates/pptx-render/src/layout.rs:1978`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1978)) and there is
no family aliasing anywhere, so `Segoe UI` never reaches a registered face under another name.

The face chosen here is the only thing that decides the rasterized weight: layout shapes with
`run.style.face.id` ([`crates/pptx-render/src/layout.rs:1383`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1383)) and `crates/pptx-raster/src/font.rs`
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
  `lower.contains("major")` ([`crates/ooxml-drawingml/src/theme.rs:203`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L203)), which `+mj-lt` does not
  satisfy, so it falls through to `get_minor_font`. project17/07/3 uses
  `<a:latin typeface="+mj-lt"/>` against a theme whose major latin is `Calibri` (registered, with
  a bold face) and whose minor latin is `Calibri Light` (not registered) — so that finding needs
  this bug fixed too, otherwise the run keeps resolving to an unregistered family. It looks like
  a distinct defect worth its own issue; it is not required for the other 14 findings.
- The canvas backend does not share the defect: [`packages/pptx/src/render/canvas.ts:234`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L234) builds
  its CSS font from the display list's `run.bold`, so the browser draws bold glyphs while layout
  measured advances on the regular face. Fixing `resolve_face` also removes that raster/canvas
  divergence.

Not confirmed: project20/05/2 also reports the title at roughly half the expected glyph height.
That is a size problem on top of the weight problem, is not explained by `resolve_face`, and
belongs with the autofit cluster.

**Suggested fix**

Make the substitute face style-aware. `SlideRenderer` keeps one `fallback: Option<FontFace>`
([`crates/pptx-render/src/layout.rs:60`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L60), set at `:111`), which is the first face registered and
carries no style. Replace it with the normalized family name of the first registration
(`fallback_family: Option<String>`) so `resolve_face` can look the fallback family up in
`self.faces` at the requested `(bold, italic)`.

`resolve_face` ([`crates/pptx-render/src/layout.rs:245`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L245)) then walks a chain that degrades family
before it degrades style, and degrades italic before weight:

1. `(family, bold, italic)`
2. `(family, bold, false)`, `(family, false, italic)`, `(family, false, false)`
3. the same four keys against `fallback_family`
4. any registered face, as today

Step 3 is what fixes all 15 findings: the harness and every real host register a bold face for
their default family, so `Segoe UI` + bold reaches Liberation Sans Bold instead of Liberation
Sans Regular.

`fallback_font()` ([`crates/pptx-render/src/layout.rs:124`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L124)) is public and used by the raster
backend for its placeholder labels; keep it returning the first registered face's id so that
callers do not change.

Step 2 still loses the weight when a host registers only the regular face of a family it does
own. Synthetic emboldening (stroking the glyph path in `crates/pptx-raster/src/font.rs`) would
cover that, but it is a separate, larger change and is not needed for any finding in this
cluster — every one of them has no face at all for the requested family.

```rust
struct SlideRenderer {
    faces: HashMap<(String, bool, bool), FontFace>,
    fallback: Option<FontFace>,          // kept for fallback_font()
    fallback_family: Option<String>,     // new: normalized family of the first registration
    ...
}

// in register_font, next to `self.fallback.get_or_insert(face)`:
self.fallback_family.get_or_insert_with(|| normalize_family(family));

fn resolve_face(&self, family: &str, bold: bool, italic: bool) -> Result<FontFace, RenderError> {
    let requested = normalize_family(family);
    let styles = [(bold, italic), (bold, false), (false, italic), (false, false)];
    for name in [Some(&requested), self.fallback_family.as_ref()].into_iter().flatten() {
        for (b, i) in styles {
            if let Some(face) = self.faces.get(&(name.clone(), b, i)) {
                return Ok(face.clone());
            }
        }
    }
    self.fallback.clone().ok_or(RenderError::NoFont)
}
```

The `+mj-lt` note in the report is a one-line change in
[`crates/ooxml-drawingml/src/theme.rs:203`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L203) (`lower.contains("major") || lower.contains("+mj")`),
but it changes font selection for every deck that uses major-font references and deserves its own
issue and its own golden review rather than riding along here.

Risks and tests to add:

- Any deck whose text currently renders in the first-registered face changes weight. That is the
  point, but it moves glyph advances, so wrap points and `spAutoFit` heights shift and every pptx
  golden image with bold text will need regenerating. Check `crates/pptx-raster/tests/golden`
  and the demo fixture snapshots.
- Preferring the requested family over the fallback family's correct style (step 2 before step 3)
  is a deliberate choice; if a host registers a family regular-only, that family keeps winning
  and bold is still lost. Document it so nobody reads the omission as an oversight.
- `fallback_font()` semantics must not change, or the raster placeholder labels
  (`crates/pptx-raster/src/font.rs` `paint_centered_label`) pick up a different face.

Tests to add, in the `layout.rs` test module: an unregistered family with `bold: true` resolves
to the fallback family's bold face; the same with `italic`; a registered family missing only its
bold face still resolves to that family; registration order does not affect any of the above.
Extend `crates/pptx-raster/tests/golden.rs` with a `bold: true` run so the raster side is pinned.

**How to verify**

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

There is no existing coverage: [`crates/pptx-render/src/layout.rs:2018`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2018) registers `Arial` in both
weights but nothing asserts what an unregistered family resolves to, and
`crates/pptx-raster/tests/golden.rs` only exercises `bold: false`. Unit tests on `resolve_face`
belong in the `layout.rs` test module; a bold golden image under
`crates/pptx-raster/tests/golden` would lock the raster side down.

**Additional context**

none.

Related issues found in the same run: none.

Files most likely involved: `crates/pptx-render/src/layout.rs`, `crates/ooxml-drawingml/src/theme.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-bold-ignored/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
