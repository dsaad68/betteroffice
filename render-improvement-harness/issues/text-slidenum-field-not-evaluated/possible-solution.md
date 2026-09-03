# Possible solution: text-slidenum-field-not-evaluated

## Approach

Evaluate the field where the display list is built, and only there. The edit layer must keep
treating a field's cached `<a:t>` as literal story text — `crates/pptx-edit/tests/write_fidelity.rs:437`
asserts exactly that, and the writer round-trips `<a:fld>` through `is_run_element`
(`crates/pptx-parse/src/write.rs:1339`) on the same assumption. Substituting at parse or story
level would break both.

Three small changes in `crates/pptx-render/src/layout.rs`:

1. Carry the number. `LayoutBuilder` (`:314`) gains `slide_number: u32`, filled in
   `layout_slide` (`:128`) from `slide_index + 1`, offset by the deck's first slide number.
2. Substitute in `content_from_body` (`:884`). It already iterates `pptx_parse::TextRun`, which
   carries `field_type` (`crates/pptx-parse/src/model.rs:332`); give the function the slide number
   and map a `slidenum` run's text to the decimal number instead of `run.text.clone()` (`:897`).
   The single call site is `:564`.
3. Read `p:presentation/@firstSlideNum` in `parse_presentation`
   (`crates/pptx-parse/src/package.rs:232`) into a new `Presentation::first_slide_num: u32`
   (default 1), so the displayed number is `first_slide_num + slide_index`. Optional — every
   harness deck omits the attribute — but it is four lines and avoids a known off-by-N.

Deliberately out of scope: the `content_from_story` path (`:862`). Slide-level fields already
carry the evaluated number in their cached text, `TextRunSnapshot` (`crates/pptx-edit/src/model.rs:58`)
has no field concept, and threading one through the Yjs story model is a much larger change with
its own edit-semantics questions (what happens when a user types inside a field). Note it in the
code so the asymmetry is not read as an oversight.

`crates/pptx-render/src/lib.rs`'s `ComposedRun` (host-composed JSON) has no field concept either;
hosts on that path pre-resolve their own text, so it needs nothing.

## Sketch

```rust
// layout.rs
struct LayoutBuilder<'a> {
    // ...
    slide_number: u32,
}

// in layout_slide, next to the other builder fields:
slide_number: package.presentation.first_slide_num
    .saturating_add(u32::try_from(slide_index).unwrap_or(u32::MAX)),

// at the single call site (:564)
let content = content_from_body(stable_id, body, self.theme, self.slide_number);

fn content_from_body(
    story_id: &str,
    body: &TextBody,
    theme: &Theme,
    slide_number: u32,
) -> TextContent {
    // ...
    .map(|run| ContentRun {
        text: match run.field_type.as_deref() {
            // Slide-level fields cache the evaluated value already and arrive
            // through content_from_story, which has no field metadata.
            Some("slidenum") => slide_number.to_string(),
            _ => run.text.clone(),
        },
        style: style_from_properties(&run.properties, theme),
    })
}
```

```rust
// package.rs, in parse_presentation
first_slide_num: root
    .attribute("firstSlideNum")
    .and_then(|value| value.parse::<u32>().ok())
    .unwrap_or(1),
```

## Risks

- Any master or layout that draws a non-placeholder `slidenum` field changes text, so its shaped
  width changes. The cisco shape is `wrap="none"` + `<a:spAutoFit/>` + `algn="r"`, so a shorter
  string moves the left edge, not the right one, and the number stays anchored where the reference
  puts it. A `algn="l"` or `algn="ctr"` field would shift; nothing in the harness decks does this,
  so it is untested.
- Adding a field to `Presentation` touches the public parse model; check
  `crates/pptx-parse/src/write.rs:563` region, which rebuilds `sldIdLst`, still round-trips the
  attribute untouched (it should — the writer patches XML rather than regenerating it, but the
  round-trip tests in `crates/pptx-edit/tests/write_fidelity.rs` are the place to confirm).
- Golden images: any pptx golden whose master carries a plain `slidenum` shape will change.
  Check `crates/pptx-raster/tests/golden` before regenerating.
- Do not "fix" this by substituting in `parse_text_run` — that would write the evaluated number
  back on save and corrupt the deck for every other consumer.

Tests to add:

- A unit test in the `layout.rs` test module: a master with a non-placeholder shape whose run is
  `<a:fld type="slidenum"><a:t>‹#›</a:t></a:fld>`, laid out at `slide_index` 0 and 2, produces
  text primitives reading `1` and `3`.
- The same deck with `firstSlideNum="7"` produces `7` and `9`.
- A non-`slidenum` field (`datetimeFigureOut`) still renders its cached text unchanged, pinning
  the deliberate scope.
- A regression assertion that `DeckSession::save()` on a deck with a `slidenum` field still
  emits the original `‹#›` cached text — extend `crates/pptx-edit/tests/write_fidelity.rs`.

## Effort

easy — one struct field, one `match` arm, one attribute parse, all in code that already has the
slide index in hand; the only real work is the new tests and any golden regeneration.
