# Possible solution: text-run-props-gradfill-not-resolved

## Approach

Teach the run-property parser about the fill choices it currently ignores, and flatten a gradient
to the one solid color the rest of the pipeline can carry.

1. In `parse_run_properties` (`crates/pptx-parse/src/drawing.rs:900`), when `a:rPr` has no
   `a:solidFill`, look for `a:gradFill` and take a representative stop. Reuse the existing
   `parse_gradient_fill` (`crates/pptx-parse/src/drawing.rs:585`) so stop parsing, `pos` clamping
   and color-modifier handling stay in one place, then pick the stop with the lowest `pos`. For the
   degenerate all-stops-equal case that every finding in this cluster uses, that is exact; for a
   real gradient it is the same approximation LibreOffice's text rendering settles on and is
   strictly better than dropping the fill.
2. Keep the flattening lossless on save. `apply_run_properties`
   (`crates/pptx-parse/src/write.rs:1520`) currently treats `color.is_some()` as "the model owns the
   fill choice" and rewrites it as `solidFill`. Add a marker so the writer can tell an authored
   `solidFill` from a flattened `gradFill` - the smallest version is a
   `#[serde(skip_serializing_if = ...)] pub color_is_gradient: bool` (or an
   `Option<GradientFill>` field, if the round-trip is expected to keep the stops) on `RunProperties`
   (`crates/pptx-parse/src/model.rs:338`), checked before the `FILL_ELEMENTS` strip at
   `crates/pptx-parse/src/write.rs:1543`: when the color came from a gradient and was not edited,
   leave the `gradFill` element untouched.

Nothing changes in `pptx-edit` or `pptx-render`: `style_from_run_properties`
(`crates/pptx-edit/src/story.rs:643`) and `resolve_style`
(`crates/pptx-render/src/layout.rs:1042`) already do the right thing once `color` is populated.

## Sketch

```rust
// crates/pptx-parse/src/drawing.rs, in parse_run_properties
color: element
    .child("solidFill")
    .and_then(parse_color_container)
    .or_else(|| run_gradient_color(element)),

/// A run `gradFill` flattened to its first stop; the display list carries one
/// colour per run, and these gradients are degenerate in practice.
fn run_gradient_color(element: &XmlElement) -> Option<ColorValue> {
    let fill = parse_gradient_fill(element.child("gradFill")?);
    let mut stops = fill.gradient?.stops;
    stops.sort_by(|a, b| a.position.total_cmp(&b.position));
    stops.into_iter().next().map(|stop| stop.color)
}
```

```rust
// crates/pptx-parse/src/write.rs, in apply_run_properties
let removed_fills: &[&str] = match (&properties.color, properties.color_is_gradient) {
    (Some(_), false) => &FILL_ELEMENTS,
    (Some(_), true) => &[],      // flattened gradFill: leave the authored element alone
    (None, _) => &["solidFill"],
};
```

## Risks

- **Round-trip regression** is the real hazard: without the writer guard, every `gradFill` on a run
  in a saved deck degrades to a `solidFill`. Add a save-and-reload test asserting the `a:gradFill`
  element and its `gsLst` survive a no-op edit, alongside one asserting an explicit colour edit
  still replaces it.
- **Over-eager flattening.** A run with a genuine two-colour gradient now paints in its first stop
  instead of the theme default. That is a visible change on decks not in this harness, but it is
  the closer approximation in every case, and none of the twelve harness decks contain a
  non-degenerate run gradient.
- Runs carrying `a:noFill` on `rPr` stay unhandled by this change (they would still fall back to
  the inherited colour instead of painting nothing). Out of scope here; worth a follow-up note.
- Tests to extend: `parses_text_formatting_and_nested_shape_types`
  (`crates/pptx-parse/src/drawing.rs:957`) for the parse side, and the `pptx-parse` write tests
  around `crates/pptx-parse/src/write.rs:1543` for the round-trip.

## Effort

easy - one new helper on an existing parse function plus a one-field guard on the writer, no model
plumbing through `pptx-edit` or `pptx-render`, and the display list needs no new paint type because
the gradients involved are degenerate.
