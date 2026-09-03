# Possible solution: text-bullets-autonum-not-drawn

## Approach

Lands on top of `text-bullets-char-indent-dropped`, which carries the bullet into
`ResolvedParagraph` and emits it as a leading run on the paragraph's first line. That plumbing is
the prerequisite; three additions make `buAutoNum` work.

**Number the paragraphs in `resolve_content`.** `resolve_content`
(`crates/pptx-render/src/layout.rs:934`) is called once per text body
(`crates/pptx-render/src/layout.rs:666`) and iterates paragraphs in document order, so it holds a
local `[u32; 9]` counter. For each paragraph whose resolved `properties.bullet` is
`Bullet::AutoNumber { scheme, start_at }`, take the paragraph's level, apply `start_at` the first
time that level is numbered, increment, reset every deeper level, and store the formatted string
on `ResolvedParagraph`. Store the resolved marker as a `String`, not the `Bullet` — layout then
treats `buChar` and `buAutoNum` identically and needs no counter state of its own. It must live
here rather than in `layout_content`, which the autofit loop re-runs
(`crates/pptx-render/src/layout.rs:686-698`) and would re-increment on each pass.

**Format the scheme.** A `fn format_autonum(value: u32, scheme: &str) -> String` splitting
`ST_TextAutonumberScheme` into a numeral style (`arabic`, `alphaLc`, `alphaUc`, `romanLc`,
`romanUc`) and a suffix (`Period`, `ParenR`, `ParenBoth`, `Plain`), defaulting to
`arabicPeriod`. `crates/docx-edit/src/bridge/mod.rs:2156-2208` already has `format_roman` and
`format_alpha`; copy them or lift them into a crate both sides depend on (`ooxml-text` is the
obvious home, since neither is DrawingML-specific).

**Parse `buFont` so the marker uses the right face.** Add `font: Option<String>` to `Bullet`
(`crates/pptx-parse/src/model.rs:320-324`), fill it from `<a:buFont typeface>` in
`parse_paragraph_properties` (`crates/pptx-parse/src/drawing.rs:853-867`), mirror it in
`write.rs` (`crates/pptx-parse/src/write.rs:1631-1646`) so round-trips stay lossless, and in the
marker's `ResolvedStyle` override `family` with it. Theme refs need no new code —
`resolve_style` already routes a leading `+` through `resolve_theme_font_ref`
(`crates/pptx-render/src/layout.rs:1031-1039`), which is exactly what `+mj-lt` needs. This is
cosmetic; it can be a follow-up if the marker-emission change is landing on its own.

Optional, and separate: `content_from_story` (`crates/pptx-render/src/layout.rs:862`) drops the
paragraph's bullet on the floor, even though `ParagraphSnapshot::bullet_json`
(`crates/pptx-edit/src/model.rs:69`) carries a serialized `Bullet`. The cascade recovers it by
paragraph index today, so the render path works; an edit that inserts or removes a paragraph
would desync those indices. Deserializing `bullet_json` into `ContentParagraph` closes that.

## Sketch

```rust
// pptx-render/src/layout.rs, resolve_content, per paragraph, after `properties`
let marker = match &properties.bullet {
    Some(Bullet::Character { value }) => Some(value.clone()),
    Some(Bullet::AutoNumber { scheme, start_at }) => {
        let level = (paragraph.level as usize).min(8);
        if numbered[level] {
            counters[level] += 1;
        } else {
            counters[level] = *start_at;
            numbered[level] = true;
        }
        counters[level + 1..].fill(0);
        numbered[level + 1..].fill(false);
        Some(format_autonum(counters[level], scheme))
    }
    Some(Bullet::None) | None => None,
};

fn format_autonum(value: u32, scheme: &str) -> String {
    let (numeral, suffix) = split_scheme(scheme);       // ("arabic", "Period"), …
    let body = match numeral {
        "alphaLc" => format_alpha(value, false),
        "alphaUc" => format_alpha(value, true),
        "romanLc" => format_roman(value, false),
        "romanUc" => format_roman(value, true),
        _ => value.to_string(),
    };
    match suffix {
        "ParenR" => format!("{body})"),
        "ParenBoth" => format!("({body})"),
        "Plain" => body,
        _ => format!("{body}."),
    }
}
```

The marker itself is placed exactly as in `text-bullets-char-indent-dropped`: shaped with the
first run's style (family overridden by `buFont`), prepended to the first line's `runs` at
`x = rect.x + marL + indent` clamped to `>= rect.x`, with `start == end` so `caret_stops` and
hit testing (`crates/pptx-render/src/layout.rs:293-300`) are untouched.

## Risks

- Wide markers (`10.`, `viii.`) overflow the `-indent` gutter and will overlap the text, as they
  do in PowerPoint. Do not widen the gutter or shift the text — evidence-2.png shows LibreOffice
  keeping `10.` and `11.` left-aligned at the same x as `1.` and leaving the text alone.
- The counter is per text body. Grouped shapes each get their own body, which is correct, but
  confirm that a shape rendered through both `content_from_story` and `content_from_body`
  (`crates/pptx-render/src/layout.rs:862`, `:884`) does not number the same body twice.
- Adding a field to `Bullet` changes its serde shape; `bullet_json` round-trips through
  `crates/pptx-edit/src/save.rs:361-368` and `crates/pptx-edit/src/story.rs:110-123`, so an
  `Option` field with `#[serde(default)]` keeps stored stories readable.
- Deeper-level reset semantics are unverified against PowerPoint (see the report). Put it behind
  a test so the chosen behaviour is at least pinned.
- Tests to add in `crates/pptx-render/src/layout.rs` (module at `:2008`): consecutive
  `arabicPeriod` paragraphs number `1.`–`3.`; a level-0 sequence interrupted by level-1
  paragraphs resumes at `2.` (the project20 slide 16 shape); `startAt="5"` starts at `5.`;
  a `buNone` paragraph draws nothing and does not consume a number.

## Effort

Medium — the counter and the scheme formatter are small and self-contained, but they sit on top
of the marker-emission work from `text-bullets-char-indent-dropped`, and `buFont` touches the
parse model, the writer and the edit story's serialized bullet.
