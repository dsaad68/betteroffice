# Possible solution: text-font-substitution-issues

## Approach

Three changes, in increasing size. The first two belong together; shipping the first alone
regresses eight of the twelve decks (see the table in `report.md`).

**1. Teach `resolve_theme_font_ref` the DrawingML token shape.**
`crates/ooxml-drawingml/src/theme.rs:191` currently decides major-vs-minor with
`lower.contains("major")` and script with `contains("eastasia") / contains("bidi") / contains("cs")`,
which only ever matches WordprocessingML's `majorAscii` / `minorHAnsi` spellings. Parse the
reference properly instead: strip a leading `+`, split on `-`, and map `mj`/`major*` -> major,
`mn`/`minor*` -> minor, `lt`/`ascii`/`hansi` -> latin, `ea`/`eastasia` -> ea, `cs`/`bidi` -> cs.
Both crates share the one function, so keep the DOCX spellings working; the DOCX tests at
`crates/docx-parse/src/theme.rs:244-247` are the regression guard for that half.

While there, make an empty `ea`/`cs` slot fall back to the latin face rather than returning `""`
(`crates/ooxml-drawingml/src/theme.rs:172-186` returns `font.ea.clone()` verbatim today) — every
theme in the corpus has `<a:ea typeface=""/>`, so without this the script fix turns 5379 `+mn-ea`
references into an empty family.

**2. Give `resolve_face` a substitution step before the blind fallback.**
`crates/pptx-render/src/layout.rs:245-257` goes straight from "family not registered" to
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
`crates/pptx-parse/src/drawing.rs:913-916` / `crates/pptx-parse/src/model.rs`, carry it through
`crates/pptx-edit/src/story.rs` into the resolved style, and in
`crates/pptx-render/src/layout.rs:1041` build a small chain `[latin_face, sym_face]` instead of a
single face. `crates/ooxml-text/src/font_store.rs:398` (`FontStore::resolve`) already picks the
first covering font for a char; splitting a run into maximal same-font subranges is what
`crates/docx-layout/src/display_list.rs:1154` does on the docx side and is the model to copy.

This third change will not clear evidence-3.png in the harness on its own: no Wingdings- or
OpenSymbol-class face ships in `packages/fonts/assets`, so `Wingdings` still resolves to nothing.
It is worth doing because it is a real parse gap, but treat it as its own issue with its own
acceptance criterion (the `sym` typeface reaches `resolve_face`), not as a diff-percentage win.

## Sketch

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

## Risks

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

## Effort

Medium: changes 1 and 2 are each a few dozen lines in one function apiece with clear unit tests,
but they must ship together and require re-diffing and re-baselining the whole corpus; change 3
(`a:sym` plus per-glyph fallback) is a separate, larger piece and should be split out rather than
bundled here.
