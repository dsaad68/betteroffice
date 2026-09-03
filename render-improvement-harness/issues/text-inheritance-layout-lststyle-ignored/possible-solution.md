# Possible solution: text-inheritance-layout-lststyle-ignored

## Approach

Add the missing shape-level tier to the cascade. Two small changes:

**Parse.** Give `TextBody` a `list_style: Vec<ParagraphProperties>` field
(`crates/pptx-parse/src/model.rs:269`) and fill it in `parse_text_body`
(`crates/pptx-parse/src/drawing.rs:764`). No new parser is needed:
`parse_style_levels` (`crates/pptx-parse/src/drawing.rs:78`) already walks `lvl1pPr`..`lvl9pPr`
children into a 9-slot `Vec<ParagraphProperties>` and returns an empty vec when there are none, which
is exactly the `lstStyle` content model and exactly the no-op an empty `<a:lstStyle/>` needs. Mark the
field `#[serde(default, skip_serializing_if = "Vec::is_empty")]` so existing snapshot JSON keeps
round-tripping.

**Render.** In `BodyCascade::paragraph_properties`
(`crates/pptx-render/src/layout.rs:808`), interleave each body's `list_style[level]` ahead of that
body's paragraph `pPr`, walking master → layout → primary. The resulting order is the ECMA-376 one:
master `txStyles` → master shape `lstStyle` → layout shape `lstStyle` → shape's own `lstStyle` →
paragraph `pPr`. Keeping the existing paragraph-`pPr` merges in place makes this additive rather than
a rewrite of the cascade.

Index by `level` only (no `.or_else(first)` fallback like `master_style` uses): an `lstStyle` that
defines only `lvl1pPr` must contribute nothing to a level-3 paragraph.

This also covers non-placeholder shapes such as project17/04's `TextBox 35/39/41`, because their
`lstStyle` arrives through `primary` in the `BodyCascade { primary: Some(body), layout: None,
master: None }` built at `crates/pptx-render/src/layout.rs:570`.

## Sketch

```rust
// crates/pptx-parse/src/model.rs
pub struct TextBody {
    // …
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list_style: Vec<ParagraphProperties>,
    pub paragraphs: Vec<TextParagraph>,
}

// crates/pptx-parse/src/drawing.rs, in parse_text_body
Ok(TextBody {
    // …
    list_style: parse_style_levels(element.child("lstStyle")),
    paragraphs,
})

// crates/pptx-render/src/layout.rs, BodyCascade::paragraph_properties
let mut properties = self
    .master_slide
    .and_then(|master| master_style(master, self.placeholder, level))
    .cloned()
    .unwrap_or_default();
for body in [self.master, self.layout, self.primary].into_iter().flatten() {
    if let Some(source) = body.list_style.get(level as usize) {
        merge_paragraph_properties(&mut properties, source);
    }
    if let Some(source) = body
        .paragraphs
        .get(index)
        .or_else(|| body.paragraphs.get(level as usize))
        .map(|paragraph| &paragraph.properties)
    {
        merge_paragraph_properties(&mut properties, source);
    }
}
properties
```

`merge_paragraph_properties` (`layout.rs:1804`) and `merge_run_properties` (`layout.rs:1825`) already
cover `algn`, `marL`, `indent`, `buNone`/`buChar`, and every `defRPr` field these decks use, so no
merge logic changes.

## Risks

- **Wide blast radius on text metrics.** Every placeholder in every deck gains a style tier it did
  not have. Expect the raster goldens to move — regenerate and eyeball
  `crates/pptx-raster/tests/golden.rs`, `golden_placeholder` (`golden.rs:328`) especially — plus
  `lays_out_demo_with_master_shapes_geometry_and_glyphs` (`layout.rs:2129`) and
  `normal_autofit_scales_text_until_the_shape_height_is_respected` (`layout.rs:2239`), where larger
  resolved sizes will change the autofit scale that gets reached.
- **Correctly larger text will now overflow.** Several of these placeholders declare `<a:noAutofit/>`
  (project20's `idx="10"`), so text that grows from 39pt to 66pt is *meant* to overflow or rewrap.
  Do not tune the fix by the total pixel diff alone; check the wrap points named in the report.
- **Ordering against the existing paragraph-`pPr` hack.** Interleaving as sketched lets a layout's
  sample-paragraph `pPr` override the same layout's `lstStyle`, which PowerPoint would not do. It is
  harmless on these decks (those `pPr`s are empty) but is the thing to revisit if a regression shows
  up; the real fix there is to stop treating layout/master sample paragraphs as a style source at all.
- **New model field crosses the wasm and Python boundaries** via `crates/betteroffice-pptx/src/types.rs`
  and the snapshot serde; `skip_serializing_if` keeps the emitted JSON unchanged for decks with no
  `lstStyle`, but run the `pptx-edit` write-fidelity tests
  (`crates/pptx-edit/tests/write_fidelity.rs`) to confirm the round-trip is untouched.
- **Does not fix colour on rollout-plan.** Those `lstStyle` `defRPr`s use `gradFill`, which
  `parse_run_properties` (`crates/pptx-parse/src/drawing.rs:917`) does not read. Sizes will be
  correct, colours still need the `gradFill` and `clrMapOvr` issues.

## Effort

Easy. `parse_style_levels`, `merge_paragraph_properties` and `BodyCascade` all exist; the change is
one struct field, one call site in the parser, and a three-line insertion in the cascade — under 30
lines. The work is in regenerating and reviewing goldens whose text metrics legitimately change.
