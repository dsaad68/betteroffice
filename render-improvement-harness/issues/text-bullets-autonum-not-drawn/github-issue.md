# pptx: buAutoNum auto-numbering not drawn

**Describe the bug**

Every paragraph carrying `<a:buAutoNum type="arabicPeriod"/>` renders with no number. The text
column itself is correct — first lines and wrapped continuation lines both sit at `marL`, exactly
where LibreOffice puts them — but the gutter to their left, where `1.` `2.` `3.` belong, is empty
(evidence-1.png, evidence-2.png). On slide 16 the same shape mixes level-0 `buAutoNum` with
level-1 `buChar`, and both markers vanish while the two indent steps survive (evidence-4.png).

This is the same downstream gap as `text-bullets-char-indent-dropped`. See that issue's report
for the shared analysis; this one covers only what `buAutoNum` needs on top of it — a counter, a
scheme formatter, and `buFont`.

The cluster's stated symptom "the hanging indent is sometimes preserved" is **not confirmed**;
it is preserved in all five findings. What is dropped is the `indent` attribute, which in these
paragraphs (`marL="514350" indent="-514350"`, identical on all five slides) positions only the
number, so its absence is invisible while no number is drawn.

Seen on 5 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project20/11** `Rectangle 7`: `1.`–`5.` missing; the text column and the wrapped `integrate with?` continuation both land where LibreOffice puts them

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-autonum-not-drawn/evidence-1.png)

**2. project20/12** eleven items: LibreOffice left-aligns `10.` and `11.` at the same gutter x as `1.`, so the marker is left-aligned at `marL + indent`, not right-aligned on the period

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-autonum-not-drawn/evidence-2.png)

**3. project20/13** nine numbered questions, all unnumbered in the candidate; the multi-line items show the text hanging indent is intact

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-autonum-not-drawn/evidence-3.png)

**4. project20/16** level-0 `buAutoNum` interleaved with six level-1 `buChar` paragraphs: LibreOffice numbers `1.` then resumes `2. 3. 4.` after the bullets, so the counter is per level and survives intervening paragraphs at another level

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-autonum-not-drawn/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 11, 12, 13, 14, 16

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(10, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**The scheme and start value are parsed correctly and then discarded in layout.** Confirmed.

`buAutoNum` is parsed at [`crates/pptx-parse/src/drawing.rs:860-866`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L860-L866) into
`Bullet::AutoNumber { scheme, start_at }` ([`crates/pptx-parse/src/model.rs:320-324`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L320-L324)), defaulting
`scheme` to `arabicPeriod` and `start_at` to `1`, and stored on `ParagraphProperties::bullet`
([`crates/pptx-parse/src/drawing.rs:876`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L876), [`crates/pptx-parse/src/model.rs:305-312`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L305-L312)). It survives
the render cascade: `BodyCascade::paragraph_properties`
([`crates/pptx-render/src/layout.rs:807-827`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L807-L827)) merges master → layout → primary and
`merge_paragraph_properties` copies `bullet` field-wise
([`crates/pptx-render/src/layout.rs:1814-1816`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1814-L1816)).

It dies at [`crates/pptx-render/src/layout.rs:994-1004`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L994-L1004). `ResolvedParagraph`
([`crates/pptx-render/src/layout.rs:910-915`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L910-L915)) has `align`, `level`, `margin_left_px` and `runs` —
no bullet, no indent. `layout_content` therefore only shifts the paragraph box right by `marL`
([`crates/pptx-render/src/layout.rs:1177-1178`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1177-L1178)) and `layout_paragraph`
([`crates/pptx-render/src/layout.rs:1192`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1192)) shapes nothing but the run text. Grepping `buAutoNum`
and `AutoNumber` across the workspace returns only the parse site, the write-back site
([`crates/pptx-parse/src/write.rs:1638-1645`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1638-L1645)) and the model — nothing in `pptx-render`,
`pptx-raster`, `ooxml-drawingml` or `ooxml-text` consumes it.

**Why this is not just "apply the buChar fix".** Three things `buChar` does not need:

1. *A counter.* The marker text depends on how many earlier paragraphs at the same level in the
   same text body were auto-numbered. `resolve_content` is called once per text body
   ([`crates/pptx-render/src/layout.rs:666`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L666)) and walks paragraphs in order, so it is the natural
   home for a `[u32; 9]` counter — and it sits outside the autofit re-layout loop
   ([`crates/pptx-render/src/layout.rs:686-698`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L686-L698)), so the numbering is computed once no matter how
   many times `layout_content` re-runs.
2. *A scheme formatter.* `ST_TextAutonumberScheme` has ~23 members (`arabicPeriod`,
   `arabicParenR`, `alphaLcParenR`, `romanUcPeriod`, …), each a numeral style plus a suffix. The
   decks here only exercise `arabicPeriod`. Prior art exists on the docx side — `format_roman`
   ([`crates/docx-edit/src/bridge/mod.rs:2156`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-edit/src/bridge/mod.rs#L2156)), `format_alpha`
   ([`crates/docx-edit/src/bridge/mod.rs:2174`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-edit/src/bridge/mod.rs#L2174)) and `format_list_counter`
   ([`crates/docx-edit/src/bridge/mod.rs:2190-2208`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-edit/src/bridge/mod.rs#L2190-L2208)) — but `docx-edit` is not a dependency of
   `pptx-render`, so it has to be reimplemented or lifted into a shared crate.
3. *`buFont`.* All five findings carry `<a:buFont typeface="+mj-lt"/>`, which for this deck's
   theme resolves to `Segoe UI Light` against `Segoe UI` for the body text
   (`render-improvement-harness/decks/project20/xml/12/theme.xml`). `buFont`, `buClr` and
   `buSzPct` are not parsed anywhere (`grep buFont crates` returns nothing) and `Bullet` has no
   room for them ([`crates/pptx-parse/src/model.rs:320-324`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L320-L324)), so the number will inherit the run's
   face. A fidelity detail here, not the cause of the blank gutter.

**Marker placement.** Measured on `decks/project20/diff-img/12-sbs.png`, LibreOffice puts the
left edge of every marker glyph at the same x, `1.` through `11.` alike (evidence-2.png), ~51 px
left of the text column at 1280 px wide, against the 54 px that `marL - (marL + indent)` =
514350 EMU predicts — the 3 px difference being the left side bearing of the digit. So the
marker is left-aligned at `rect.x + marL + indent`, the same position the `buChar` solution
derives, and nothing about the text box moves.

**Not needed for this cluster:** the shape-level `<a:lstStyle>` parse gap from
`text-bullets-char-indent-dropped`. All five failing shapes are non-placeholder `Rectangle 7`s
with an empty `<a:lstStyle/>` and the `pPr` written on each `<a:p>`, so the property is already
reachable through `cascade.primary`.

**Unconfirmed:** what PowerPoint does when a shallower level increments after a deeper one has
been numbered (the docx implementation resets every deeper counter,
[`crates/docx-edit/src/bridge/mod.rs:2311-2313`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-edit/src/bridge/mod.rs#L2311-L2313)). Slide 16 only interleaves `buAutoNum` with
`buChar`, so these decks do not exercise it. Likewise `startAt` — no finding in this cluster
carries the attribute.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Lands on top of `text-bullets-char-indent-dropped`, which carries the bullet into
`ResolvedParagraph` and emits it as a leading run on the paragraph's first line. That plumbing is
the prerequisite; three additions make `buAutoNum` work.

**Number the paragraphs in `resolve_content`.** `resolve_content`
([`crates/pptx-render/src/layout.rs:934`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L934)) is called once per text body
([`crates/pptx-render/src/layout.rs:666`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L666)) and iterates paragraphs in document order, so it holds a
local `[u32; 9]` counter. For each paragraph whose resolved `properties.bullet` is
`Bullet::AutoNumber { scheme, start_at }`, take the paragraph's level, apply `start_at` the first
time that level is numbered, increment, reset every deeper level, and store the formatted string
on `ResolvedParagraph`. Store the resolved marker as a `String`, not the `Bullet` — layout then
treats `buChar` and `buAutoNum` identically and needs no counter state of its own. It must live
here rather than in `layout_content`, which the autofit loop re-runs
([`crates/pptx-render/src/layout.rs:686-698`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L686-L698)) and would re-increment on each pass.

**Format the scheme.** A `fn format_autonum(value: u32, scheme: &str) -> String` splitting
`ST_TextAutonumberScheme` into a numeral style (`arabic`, `alphaLc`, `alphaUc`, `romanLc`,
`romanUc`) and a suffix (`Period`, `ParenR`, `ParenBoth`, `Plain`), defaulting to
`arabicPeriod`. [`crates/docx-edit/src/bridge/mod.rs:2156-2208`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-edit/src/bridge/mod.rs#L2156-L2208) already has `format_roman` and
`format_alpha`; copy them or lift them into a crate both sides depend on (`ooxml-text` is the
obvious home, since neither is DrawingML-specific).

**Parse `buFont` so the marker uses the right face.** Add `font: Option<String>` to `Bullet`
([`crates/pptx-parse/src/model.rs:320-324`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L320-L324)), fill it from `<a:buFont typeface>` in
`parse_paragraph_properties` ([`crates/pptx-parse/src/drawing.rs:853-867`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L853-L867)), mirror it in
`write.rs` ([`crates/pptx-parse/src/write.rs:1631-1646`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1631-L1646)) so round-trips stay lossless, and in the
marker's `ResolvedStyle` override `family` with it. Theme refs need no new code —
`resolve_style` already routes a leading `+` through `resolve_theme_font_ref`
([`crates/pptx-render/src/layout.rs:1031-1039`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1031-L1039)), which is exactly what `+mj-lt` needs. This is
cosmetic; it can be a follow-up if the marker-emission change is landing on its own.

Optional, and separate: `content_from_story` ([`crates/pptx-render/src/layout.rs:862`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L862)) drops the
paragraph's bullet on the floor, even though `ParagraphSnapshot::bullet_json`
([`crates/pptx-edit/src/model.rs:69`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L69)) carries a serialized `Bullet`. The cascade recovers it by
paragraph index today, so the render path works; an edit that inserts or removes a paragraph
would desync those indices. Deserializing `bullet_json` into `ContentParagraph` closes that.

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
hit testing ([`crates/pptx-render/src/layout.rs:293-300`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L293-L300)) are untouched.

Risks and tests to add:

- Wide markers (`10.`, `viii.`) overflow the `-indent` gutter and will overlap the text, as they
  do in PowerPoint. Do not widen the gutter or shift the text — evidence-2.png shows LibreOffice
  keeping `10.` and `11.` left-aligned at the same x as `1.` and leaving the text alone.
- The counter is per text body. Grouped shapes each get their own body, which is correct, but
  confirm that a shape rendered through both `content_from_story` and `content_from_body`
  ([`crates/pptx-render/src/layout.rs:862`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L862), `:884`) does not number the same body twice.
- Adding a field to `Bullet` changes its serde shape; `bullet_json` round-trips through
  [`crates/pptx-edit/src/save.rs:361-368`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L361-L368) and [`crates/pptx-edit/src/story.rs:110-123`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L110-L123), so an
  `Option` field with `#[serde(default)]` keeps stored stories readable.
- Deeper-level reset semantics are unverified against PowerPoint (see the report). Put it behind
  a test so the chosen behaviour is at least pinned.
- Tests to add in `crates/pptx-render/src/layout.rs` (module at `:2008`): consecutive
  `arabicPeriod` paragraphs number `1.`–`3.`; a level-0 sequence interrupted by level-1
  paragraphs resumes at `2.` (the project20 slide 16 shape); `startAt="5"` starts at `5.`;
  a `buNone` paragraph draws nothing and does not consume a number.

**How to verify**

Re-render `project20` slides 11, 12, 13, 14 and 16 with
`render-improvement-harness/scripts/render_bo.py` and re-run `diff.py`. The number column should
fill in and no text should move. Current fine diffs: `11` 4.50%, `12` 11.66%, `13` 13.93%,
`14` 3.86%, `16` 6.02%. Expect a couple of points off `12` and `13`; the rest of those two slides
is a separate line-height defect (`project20/13/4`, `project20/16/4`) and unapplied bold
(`project20/11/2`, `project20/12/2`, `project20/14/1`), which will keep the residual high.

No existing test covers bullets or auto-numbering: the `layout.rs` test module
([`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2008) onward) has no `marL`, bullet or `buAutoNum` case. Add
unit tests there for the counter across consecutive paragraphs, for a counter that resumes after
an intervening paragraph at another level (the slide-16 shape), and for `arabicPeriod` marker
text and placement at `rect.x + marL + indent`.

**Additional context**

none.

Related issues found in the same run: `text-bullets-char-indent-dropped`

Files most likely involved: `crates/pptx-render/src/layout.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/drawing.rs`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-autonum-not-drawn/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
