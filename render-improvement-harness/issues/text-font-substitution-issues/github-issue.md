# pptx: Theme "+mj-lt" resolves to the minor font, so major-font runs fall back to a wider face

**Describe the bug**

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

Seen on 8 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project17/12** Title: reference fits two lines in the band, candidate wraps at "Middle East" and drops line 2 into the white body

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-font-substitution-issues/evidence-1.png)

**2. project17/12** Left-column question boxes: candidate needs one extra line per box, last line sits on/past the border

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-font-substitution-issues/evidence-2.png)

**3. project17/02** Timeline bullets: reference draws the Wingdings arrow and the Calibri "ti" ligature, candidate draws tofu and unligated, wider glyphs

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-font-substitution-issues/evidence-3.png)

**4. project17/12** Same build, three ways: reference, candidate today, candidate with Carlito additionally registered as `Calibri Light` - the wrap point snaps back to the reference's

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-font-substitution-issues/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 5, 10, 11, 12

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
([`crates/pptx-parse/src/theme.rs:60`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/theme.rs#L60)). The defect is in the resolver.
[`crates/pptx-render/src/layout.rs:1034`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1034) routes any family starting with `+` through
`resolve_theme_font_ref` ([`crates/ooxml-drawingml/src/theme.rs:191`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L191)), whose major/minor decision is

```rust
if lower.contains("major") {          // theme.rs:203
    get_major_font(theme, script)
} else {
    get_minor_font(theme, script)
}
```

`resolve_theme_font_ref` was written for WordprocessingML, whose references spell out
`majorAscii` / `minorHAnsi` (its only tests use those forms -
[`crates/docx-parse/src/theme.rs:244`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/theme.rs#L244)). DrawingML spells the same thing `+mj-lt` / `+mn-lt`, which
does not contain the substring `major`, so **every** `+mj-*` reference in a pptx falls through to
`get_minor_font` ([`crates/ooxml-drawingml/src/theme.rs:166`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L166)).

For project17 that turns `+mj-lt` into `Calibri Light`. The harness registers Carlito under the
names `Calibri` and `Carlito` only (`render-improvement-harness/scripts/render_bo.py:15`), so
`Calibri Light` misses both lookups in `resolve_face`
([`crates/pptx-render/src/layout.rs:245-257`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L245-L257)) - `normalize_family` is a lowercase-and-trim with no
aliasing ([`crates/pptx-render/src/layout.rs:1978`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1978)) - and the run lands on `self.fallback`, the
first face ever registered ([`crates/pptx-render/src/layout.rs:111`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L111)), which for this harness is
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
([`crates/pptx-parse/src/drawing.rs:913-916`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L913-L916)); there is no `ea`, `cs` or `sym` on
`RunProperties`. `"sym"` appears in the crates only as an element name the writer preserves in
document order ([`crates/pptx-parse/src/write.rs:1511`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1511)).

The slide-02 bullet run is

```xml
<a:rPr sz="1400" ...><a:latin typeface="Sakkal Majalla"/>
  <a:sym typeface="Wingdings" charset="2"/></a:rPr>
<a:t>&#xF0E8;</a:t>
```

so the renderer shapes U+F0E8 with the *latin* face, gets glyph 0, and paints the `.notdef` box.
There is also no glyph-level fallback on the pptx path: `shape` takes a single `FontId`
([`crates/ooxml-text/src/shape.rs:52`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/shape.rs#L52)) and [`crates/pptx-render/src/layout.rs:1041`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1041) hands it one
resolved face, so the chain resolver that already exists
([`crates/ooxml-text/src/font_store.rs:398`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/font_store.rs#L398)) is never used here. docx has per-run fallback chains
([`crates/docx-layout/src/display_list.rs:1154`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/display_list.rs#L1154)); pptx does not.

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
`bidi` ([`crates/ooxml-drawingml/src/theme.rs:196-199`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L196-L199)), so `+mn-ea` and `+mj-ea` resolve as
*latin*. `+mn-cs` happens to work only because `"+mn-cs"` contains the substring `cs`. The corpus
has 5379 `+mn-ea` references, but every theme in it leaves `<a:ea typeface=""/>` empty, so
correcting the script today would resolve them to an empty family. Worth fixing together with the
major/minor test, with the empty-string case falling back to latin.

**Suggested fix**

Three changes, in increasing size. The first two belong together; shipping the first alone
regresses eight of the twelve decks (see the table in `report.md`).

**1. Teach `resolve_theme_font_ref` the DrawingML token shape.**
[`crates/ooxml-drawingml/src/theme.rs:191`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L191) currently decides major-vs-minor with
`lower.contains("major")` and script with `contains("eastasia") / contains("bidi") / contains("cs")`,
which only ever matches WordprocessingML's `majorAscii` / `minorHAnsi` spellings. Parse the
reference properly instead: strip a leading `+`, split on `-`, and map `mj`/`major*` -> major,
`mn`/`minor*` -> minor, `lt`/`ascii`/`hansi` -> latin, `ea`/`eastasia` -> ea, `cs`/`bidi` -> cs.
Both crates share the one function, so keep the DOCX spellings working; the DOCX tests at
[`crates/docx-parse/src/theme.rs:244-247`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/theme.rs#L244-L247) are the regression guard for that half.

While there, make an empty `ea`/`cs` slot fall back to the latin face rather than returning `""`
([`crates/ooxml-drawingml/src/theme.rs:172-186`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L172-L186) returns `font.ea.clone()` verbatim today) — every
theme in the corpus has `<a:ea typeface=""/>`, so without this the script fix turns 5379 `+mn-ea`
references into an empty family.

**2. Give `resolve_face` a substitution step before the blind fallback.**
[`crates/pptx-render/src/layout.rs:245-257`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L245-L257) goes straight from "family not registered" to
`self.fallback`, the first face ever registered. Insert one generic step between them: retry the
family with a trailing weight/width qualifier removed (`Light`, `Semilight`, `Semibold`, `Medium`,
`Black`, `Display`, `Heavy`, `Thin`), mapping a stripped `Light`/`Thin`/`Semilight` to the regular
weight and `Semibold`/`Black`/`Heavy` to bold. `Calibri Light` then finds the registered `Calibri`,
`Segoe UI Semibold` finds `Segoe UI`, and no font-name database is needed. This is the step that
keeps ocp-psp-plan and the chart decks from regressing when change 1 lands, and it also picks up
part of `text-run-props-bold-ignored` (`Calibri Light`, `Segoe UI Semibold` are named there).

Keep this ordered *after* the exact `(family, bold, italic)` lookup and *before* `self.fallback`,
and fix the fallback itself the way `text-run-props-bold-ignored` describes (a per-(bold, italic)
fallback rather than one face), so the two issues compose instead of fighting.

**3. Parse `<a:sym>` and use it for uncovered code points.**
Add `symbol_font: Option<String>` next to `font_family` in
[`crates/pptx-parse/src/drawing.rs:913-916`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L913-L916) / `crates/pptx-parse/src/model.rs`, carry it through
`crates/pptx-edit/src/story.rs` into the resolved style, and in
[`crates/pptx-render/src/layout.rs:1041`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1041) build a small chain `[latin_face, sym_face]` instead of a
single face. [`crates/ooxml-text/src/font_store.rs:398`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/font_store.rs#L398) (`FontStore::resolve`) already picks the
first covering font for a char; splitting a run into maximal same-font subranges is what
[`crates/docx-layout/src/display_list.rs:1154`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/display_list.rs#L1154) does on the docx side and is the model to copy.

This third change will not clear evidence-3.png in the harness on its own: no Wingdings- or
OpenSymbol-class face ships in `packages/fonts/assets`, so `Wingdings` still resolves to nothing.
It is worth doing because it is a real parse gap, but treat it as its own issue with its own
acceptance criterion (the `sym` typeface reaches `resolve_face`), not as a diff-percentage win.

```rust
// crates/ooxml-drawingml/src/theme.rs
pub fn resolve_theme_font_ref(theme: Option<&Theme>, reference: &str) -> String {
    let lower = reference.trim_start_matches('+').to_ascii_lowercase();
    let (slot, script) = match lower.split_once('-') {
        // DrawingML: "mj-lt", "mn-ea", "mj-cs"
        Some((slot @ ("mj" | "mn"), script)) => (slot == "mj", script),
        // WordprocessingML: "majorAscii", "minorEastAsia", "minorBidi"
        _ => (
            lower.starts_with("major"),
            lower.trim_start_matches("major").trim_start_matches("minor"),
        ),
    };
    let script = match script {
        "ea" | "eastasia" => "ea",
        "cs" | "bidi" => "cs",
        _ => "latin",
    };
    if slot { get_major_font(theme, script) } else { get_minor_font(theme, script) }
}

// crates/pptx-render/src/layout.rs
const QUALIFIERS: &[(&str, Option<bool>)] = &[
    ("light", Some(false)), ("semilight", Some(false)), ("thin", Some(false)),
    ("semibold", Some(true)), ("black", Some(true)), ("heavy", Some(true)),
    ("medium", None), ("display", None),
];

fn resolve_face(&self, family: &str, bold: bool, italic: bool) -> Result<FontFace, RenderError> {
    let normalized = normalize_family(family);
    if let Some(face) = self.faces.get(&(normalized.clone(), bold, italic)) {
        return Ok(face.clone());
    }
    // strip "Calibri Light" -> "Calibri", carrying the implied weight
    if let Some((base, weight)) = strip_qualifier(&normalized) {
        let want = weight.unwrap_or(bold);
        if let Some(face) = self.faces.get(&(base.clone(), want, italic))
            .or_else(|| self.faces.get(&(base, bold, italic)))
        {
            return Ok(face.clone());
        }
    }
    self.faces.get(&(normalized, false, false))
        .or_else(|| self.weighted_fallback(bold, italic))  // see text-run-props-bold-ignored
        .cloned()
        .ok_or(RenderError::NoFont)
}
```

Risks and tests to add:

- **Corpus-wide rendering change.** `+mj-lt` appears 684 times across the decks and every existing
  pptx golden that uses a theme font is a candidate for churn. Land changes 1 and 2 in one commit
  and re-diff all twelve decks, not just project17.
- **The qualifier strip is a heuristic.** `Segoe UI Light` -> `Segoe UI` is a weight change, not an
  identity; it is nonetheless what LibreOffice and PowerPoint effectively do, and strictly better
  than today's "whatever was registered first". Keep the list short and the match anchored to the
  end of the family name so `Light Serif`-style real family names are not mangled.
- **Interaction with `text-run-props-bold-ignored`.** Both issues edit `resolve_face`. Do the
  weighted-fallback change first, then layer the substitution step on top; the sketch above assumes
  that order.
- **Tests to add**: unit tests in `crates/ooxml-drawingml/src/theme.rs` for `+mj-lt`, `+mn-lt`,
  `+mj-ea`, `+mn-cs` against a theme whose major and minor differ (project17's inverted scheme is
  the interesting case, and the existing test at line 247 must keep passing); unit tests on
  `resolve_face` in `crates/pptx-render/src/layout.rs` asserting `Calibri Light` reaches a
  registered `Calibri` face and that an unrelated unregistered family still falls back; a
  `crates/pptx-raster/tests/golden` case rendering a `+mj-lt` run against a two-family theme.

**How to verify**

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

Existing coverage: none. [`crates/ooxml-drawingml/src/theme.rs:247`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/theme.rs#L247) only asserts the Office
defaults through `get_major_font`/`get_minor_font` directly, never through
`resolve_theme_font_ref`; the only `resolve_theme_font_ref` tests are the DOCX-form ones at
[`crates/docx-parse/src/theme.rs:244-247`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/theme.rs#L244-L247). `crates/pptx-render` never mentions `+mj-lt` outside
[`crates/pptx-render/src/layout.rs:1040`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1040), which passes `+mn-lt`.

**Additional context**

none.

Related issues found in the same run: `text-layout-master-lnspc-ignored`, `text-overflow-autofit-not-handled`, #266

Files most likely involved: `crates/ooxml-drawingml/src/theme.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-parse/src/drawing.rs`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-font-substitution-issues/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
