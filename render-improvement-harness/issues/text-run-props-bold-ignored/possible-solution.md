# Possible solution: text-run-props-bold-ignored

## Approach

Make the substitute face style-aware. `SlideRenderer` keeps one `fallback: Option<FontFace>`
(`crates/pptx-render/src/layout.rs:60`, set at `:111`), which is the first face registered and
carries no style. Replace it with the normalized family name of the first registration
(`fallback_family: Option<String>`) so `resolve_face` can look the fallback family up in
`self.faces` at the requested `(bold, italic)`.

`resolve_face` (`crates/pptx-render/src/layout.rs:245`) then walks a chain that degrades family
before it degrades style, and degrades italic before weight:

1. `(family, bold, italic)`
2. `(family, bold, false)`, `(family, false, italic)`, `(family, false, false)`
3. the same four keys against `fallback_family`
4. any registered face, as today

Step 3 is what fixes all 15 findings: the harness and every real host register a bold face for
their default family, so `Segoe UI` + bold reaches Liberation Sans Bold instead of Liberation
Sans Regular.

`fallback_font()` (`crates/pptx-render/src/layout.rs:124`) is public and used by the raster
backend for its placeholder labels; keep it returning the first registered face's id so that
callers do not change.

Step 2 still loses the weight when a host registers only the regular face of a family it does
own. Synthetic emboldening (stroking the glyph path in `crates/pptx-raster/src/font.rs`) would
cover that, but it is a separate, larger change and is not needed for any finding in this
cluster — every one of them has no face at all for the requested family.

## Sketch

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
`crates/ooxml-drawingml/src/theme.rs:203` (`lower.contains("major") || lower.contains("+mj")`),
but it changes font selection for every deck that uses major-font references and deserves its own
issue and its own golden review rather than riding along here.

## Risks

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

## Effort

easy — one function plus one struct field in `crates/pptx-render/src/layout.rs`, with the real
work being regenerated golden images.
