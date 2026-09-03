# Possible solution: text-bullets-char-indent-dropped

## Approach

Three changes, in dependency order.

**Parse the shape's `<a:lstStyle>`.** Give `TextBody` (`crates/pptx-parse/src/model.rs:269-278`)
a `list_style: Vec<ParagraphProperties>` and fill it in `parse_text_body`
(`crates/pptx-parse/src/drawing.rs:764-789`) by reusing `parse_style_levels`
(`crates/pptx-parse/src/drawing.rs:78`), which already maps `lvl1pPr`..`lvl9pPr` onto a
nine-slot vector. `parse_style_levels` is private to `drawing.rs`, so no visibility change is
needed. `text-inheritance-layout-lststyle-ignored` needs this same field, so land it once.

**Consult it in the cascade.** In `BodyCascade::paragraph_properties`
(`crates/pptx-render/src/layout.rs:808-828`), merge each body's `list_style[level]` before that
body's own paragraph, keeping master → layout → primary order. The existing
`merge_paragraph_properties` (`crates/pptx-render/src/layout.rs:1804`) already does the
field-wise override, so nothing else moves.

**Carry the bullet into layout and emit it as a run.** Add `bullet: Option<Bullet>` and
`indent_px: f32` to `ResolvedParagraph` (`crates/pptx-render/src/layout.rs:910-915`), populated
at `crates/pptx-render/src/layout.rs:994-1004` next to `margin_left_px`. In `layout_paragraph`
(`crates/pptx-render/src/layout.rs:1192`), when the paragraph resolves to
`Bullet::Character` and has at least one non-empty line, shape the character with the first
run's style through the existing `add_shaped_segment` path and prepend the resulting
`PositionedTextRun` to the first line's `runs` at `x = rect.x + marL + indent` (clamped to
`>= rect.x`). Leave `line.x`, `line.width`, `line.start`/`end` and `caret_stops` untouched: hit
testing reads only `caret_stops` (`crates/pptx-render/src/layout.rs:296`), so the bullet stays
out of the story's character space. `paint_lines`
(`crates/pptx-raster/src/font.rs:47-68`) needs no change.

`Bullet::None` must suppress an inherited bullet — it already does, because
`merge_paragraph_properties` overwrites with `Some(Bullet::None)`.

Leave `Bullet::AutoNumber` alone here; it is tracked as `text-bullets-autonum-not-drawn` and
lands on the same plumbing once this is in.

## Sketch

```rust
// pptx-parse/src/drawing.rs, parse_text_body
list_style: parse_style_levels(element.child("lstStyle")),

// pptx-render/src/layout.rs, BodyCascade::paragraph_properties
for body in [self.master, self.layout, self.primary].into_iter().flatten() {
    if let Some(source) = body.list_style.get(level as usize) {
        merge_paragraph_properties(&mut properties, source);
    }
    if let Some(source) = body.paragraphs.get(index)./* … as today */ {
        merge_paragraph_properties(&mut properties, source);
    }
}

// pptx-render/src/layout.rs, layout_paragraph, after the first line is built
if let Some(Bullet::Character { value }) = &paragraph.bullet {
    let style = &paragraph.runs[0].style;
    let mut marker = shape_marker(fonts, value, style, scale)?;   // reuses add_shaped_segment
    let marker_x = (x + paragraph.indent_px).max(x - paragraph.margin_left_px);
    place_at(&mut marker, marker_x, first.baseline);
    first.runs.insert(0, marker);                                  // start == end, no caret stop
}
```

## Risks

- The bullet run has `start == end`, so any consumer that reconstructs story text by
  concatenating `line.runs[*].text` would gain a stray glyph. `packages/pptx` and
  `packages/pptx-react` paint from `lines`; check `packages/pptx/src/render/canvas.test.ts` and
  `packages/pptx-react/src/interactions.ts` for such a reconstruction before landing.
- `margin_left_px` is not multiplied by `scale` today
  (`crates/pptx-render/src/layout.rs:1002` vs the autofit loop at
  `crates/pptx-render/src/layout.rs:686-698`); the bullet offset will inherit the same
  inconsistency. Worth fixing in the same change, but it moves text on autofit shapes, so gate
  it behind its own test.
- `buFont`/`buClr`/`buSzPct` are still unmodelled, so the bullet inherits the run's face, colour
  and size. Fine for the Arial `•`/`▪`/`–` in these decks; wrong for Wingdings dingbat decks,
  which will render a Latin letter instead of a symbol. Follow-up, not a blocker.
- Shapes whose paragraphs inherit a bullet they previously did not get will now be one glyph
  wider on the first line; nothing wraps differently because the bullet sits in the
  `marL`-to-`marL+indent` gutter, outside the wrap width.
- Tests to add in `crates/pptx-render/src/layout.rs`'s test module: a `buChar` paragraph places
  the glyph at `rect.x + marL + indent` and leaves `caret_stops` unchanged; a `buNone` override
  on a level whose `lstStyle` defines a bullet draws nothing; a shape-level `lstStyle` bullet
  reaches the layout.

## Effort

Medium — the parse-side field and the cascade hook are small and mechanical, but emitting a
non-story run into an existing positioned line needs care around `caret_stops`, hit testing, and
the TypeScript consumers of the display list.
