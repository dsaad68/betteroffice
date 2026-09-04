# pptx: Draw buChar bullet glyphs and honour the paragraph hanging indent

**Describe the bug**

Every paragraph that carries a `buChar` bullet renders with no marker at all. The list text
itself lands in the right place — `marL` is applied, so first lines and wrapped continuation
lines sit where LibreOffice puts them — but the glyph column to their left is empty, so a
multi-level list collapses into an undifferentiated block of text (evidence-1.png: `•` at
level 1 and `–` at level 2 both vanish, while the two indent steps survive). The same happens
whether the bullet is declared on the paragraph's own `pPr` (evidence-3.png, evidence-4.png)
or inherited from the shape's `<a:lstStyle>` (evidence-2.png).

The cluster's original title also claims the hanging indent is dropped. That half is **not
confirmed**: in all four evidence slides the text columns line up with the reference to within
a pixel or two. What is genuinely dropped is the `indent` attribute, which in these decks only
governs where the *bullet* sits (it is `-marL`, or close to it, in every failing paragraph), so
its absence is invisible while no bullet is drawn at all.

Seen on 9 slides across 4 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project17/06** `Rectangle 8`: level-1 `•` and level-2 `–` both missing; the two `marL` steps are still applied, so the text columns match the reference

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-char-indent-dropped/evidence-1.png)

**2. project17/07** `Rectangle 17`: the `▪` comes from the shape's own `<a:lstStyle>/<a:lvl2pPr>`, which is never parsed — the bullet is absent from the model, not just from the paint

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-char-indent-dropped/evidence-2.png)

**3. rollout-plan/02** `Rectangle 5`: bullet declared directly on `pPr` (`marL=342900 indent=-342900`); text starts at `marL` as it should, the `•` column is blank

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-char-indent-dropped/evidence-3.png)

**4. rollout-plan/08** `Rectangle 10`: wrapped continuation lines align with their first line in both renderers, showing the hanging indent is not the defect — only the glyph is

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-char-indent-dropped/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 2
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 5, 6, 7, 10
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 4
- `rollout-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 5, 8

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

Two separate gaps, both confirmed against the XML.

**1. The bullet is parsed, cascaded, and then thrown away (7 of 9 findings).**

`buNone` / `buChar` / `buAutoNum` are parsed into `ParagraphProperties::bullet` at
[`crates/pptx-parse/src/drawing.rs:853-867`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L853-L867), stored at [`crates/pptx-parse/src/drawing.rs:876`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L876),
with `marL` / `indent` alongside at [`crates/pptx-parse/src/drawing.rs:874-875`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L874-L875). The render
cascade merges all three through the master → layout → slide chain
([`crates/pptx-render/src/layout.rs:808-828`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L808-L828), field-by-field merge at
[`crates/pptx-render/src/layout.rs:1808-1816`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1808-L1816)).

The value dies at [`crates/pptx-render/src/layout.rs:994-1004`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L994-L1004): `ResolvedParagraph`
([`crates/pptx-render/src/layout.rs:910-915`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L910-L915)) has exactly one geometry field, `margin_left_px`,
and no bullet field at all. `layout_content` therefore only shifts the paragraph box right by
`marL` ([`crates/pptx-render/src/layout.rs:1177-1178`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1177-L1178)) and `layout_paragraph` shapes nothing but
the run text ([`crates/pptx-render/src/layout.rs:1192-1265`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1192-L1265)). Grepping `bullet` across
`crates/pptx-render`, `crates/pptx-raster`, `crates/ooxml-drawingml` and `crates/ooxml-text`
returns only the two merge lines above — no consumer exists downstream. Parsed and ignored.

**2. A shape's own `<a:lstStyle>` is never parsed (project17/07/2, project17/10/5).**

`parse_text_body` ([`crates/pptx-parse/src/drawing.rs:764-789`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L764-L789)) reads `bodyPr` and the `<a:p>`
children and nothing else; `TextBody` ([`crates/pptx-parse/src/model.rs:269-278`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L269-L278)) has no
list-style field. In `decks/project17/xml/07/slide.xml` the `▪` for `Rectangle 17` lives in that
shape's `<a:lstStyle><a:lvl2pPr marL="193675" indent="-192088" …><a:buChar char="▪"/>`, and in
`decks/project17/xml/10/slide.xml` `TextBox 60` carries the same construct. For those two
findings the bullet is never parsed, so fixing (1) alone leaves them blank. This overlaps with
`text-inheritance-layout-lststyle-ignored`, which needs the same parse-side field for the
*layout* placeholder's `lstStyle`; landing the parse change once serves both.

Secondary, lower-confidence gap: `Bullet` ([`crates/pptx-parse/src/model.rs:320-324`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L320-L324)) carries
only the character, so `buFont`, `buClr` and `buSzPct` have nowhere to go. The failing decks all
use `buFont typeface="Arial"` with real Unicode characters and `buClr` values that resolve close
to the text colour, so this is a fidelity detail rather than the cause of the blank column — but
a bullet drawn in the run's own font at the run's own size will be visibly wrong for decks that
use Wingdings dingbats.

Painting is not the problem: `paint_lines` ([`crates/pptx-raster/src/font.rs:47-68`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/font.rs#L47-L68)) walks
`line.runs`, so a synthetic bullet run appended to the first line of each paragraph renders with
no raster change.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Three changes, in dependency order.

**Parse the shape's `<a:lstStyle>`.** Give `TextBody` ([`crates/pptx-parse/src/model.rs:269-278`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L269-L278))
a `list_style: Vec<ParagraphProperties>` and fill it in `parse_text_body`
([`crates/pptx-parse/src/drawing.rs:764-789`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L764-L789)) by reusing `parse_style_levels`
([`crates/pptx-parse/src/drawing.rs:78`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L78)), which already maps `lvl1pPr`..`lvl9pPr` onto a
nine-slot vector. `parse_style_levels` is private to `drawing.rs`, so no visibility change is
needed. `text-inheritance-layout-lststyle-ignored` needs this same field, so land it once.

**Consult it in the cascade.** In `BodyCascade::paragraph_properties`
([`crates/pptx-render/src/layout.rs:808-828`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L808-L828)), merge each body's `list_style[level]` before that
body's own paragraph, keeping master → layout → primary order. The existing
`merge_paragraph_properties` ([`crates/pptx-render/src/layout.rs:1804`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1804)) already does the
field-wise override, so nothing else moves.

**Carry the bullet into layout and emit it as a run.** Add `bullet: Option<Bullet>` and
`indent_px: f32` to `ResolvedParagraph` ([`crates/pptx-render/src/layout.rs:910-915`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L910-L915)), populated
at [`crates/pptx-render/src/layout.rs:994-1004`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L994-L1004) next to `margin_left_px`. In `layout_paragraph`
([`crates/pptx-render/src/layout.rs:1192`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1192)), when the paragraph resolves to
`Bullet::Character` and has at least one non-empty line, shape the character with the first
run's style through the existing `add_shaped_segment` path and prepend the resulting
`PositionedTextRun` to the first line's `runs` at `x = rect.x + marL + indent` (clamped to
`>= rect.x`). Leave `line.x`, `line.width`, `line.start`/`end` and `caret_stops` untouched: hit
testing reads only `caret_stops` ([`crates/pptx-render/src/layout.rs:296`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L296)), so the bullet stays
out of the story's character space. `paint_lines`
([`crates/pptx-raster/src/font.rs:47-68`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/font.rs#L47-L68)) needs no change.

`Bullet::None` must suppress an inherited bullet — it already does, because
`merge_paragraph_properties` overwrites with `Some(Bullet::None)`.

Leave `Bullet::AutoNumber` alone here; it is tracked as `text-bullets-autonum-not-drawn` and
lands on the same plumbing once this is in.

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

Risks and tests to add:

- The bullet run has `start == end`, so any consumer that reconstructs story text by
  concatenating `line.runs[*].text` would gain a stray glyph. `packages/pptx` and
  `packages/pptx-react` paint from `lines`; check `packages/pptx/src/render/canvas.test.ts` and
  `packages/pptx-react/src/interactions.ts` for such a reconstruction before landing.
- `margin_left_px` is not multiplied by `scale` today
  ([`crates/pptx-render/src/layout.rs:1002`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1002) vs the autofit loop at
  [`crates/pptx-render/src/layout.rs:686-698`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L686-L698)); the bullet offset will inherit the same
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

**How to verify**

Re-render `project17` slides 5, 6, 7, 10; `rollout-plan` slides 2, 5, 8; `ocp-psp-plan` slide 2;
`project20` slide 4 with `render-improvement-harness/scripts/render_bo.py` and re-run `diff.py`.
The bullet column should fill in and the text columns should not move. Expected drops:
`project17/06` 13.99% and `project17/07` 13.83% should fall by several points each;
`rollout-plan/02` 4.79% should drop toward the noise floor. `project20/04` will improve only
partly — that slide also loses its `spcBef`/`spcAft` paragraph gaps, a different defect.

No existing test covers bullets: the test module at [`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2008)
onward has no bullet or `marL` case, and grepping `margin_left` across `crates/pptx-render`
finds only production code. A new unit test there should assert that a paragraph with
`marL`/`indent`/`buChar` produces a first line whose runs begin with the bullet glyph at
`rect.x + marL + indent`, that the text run still starts at `rect.x + marL`, and that
`caret_stops` are unchanged so hit testing ([`crates/pptx-render/src/layout.rs:296`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L296)) does not
shift.

**Additional context**

none.

Related issues found in the same run: `text-bullets-autonum-not-drawn`, `text-inheritance-layout-lststyle-ignored`

Files most likely involved: `crates/pptx-render/src/layout.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/drawing.rs`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-bullets-char-indent-dropped/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
