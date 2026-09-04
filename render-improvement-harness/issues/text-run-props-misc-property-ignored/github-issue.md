# pptx: Misc run properties ignored (superscript baseline, italic)

**Describe the bug**

Two unrelated run properties are lost, and they have two different causes.

`baseline="30000"` — the superscript flag — does nothing. The footnote marker after
"75% of mobile apps fail basic security tests" is drawn at the full 12pt run size, sitting on the
same baseline as the sentence it annotates, so the sentence reads "…security tests1"
(evidence-1.png). The reference draws it small and raised.

`i="1"` also does nothing on the four Incentives-pod role subtitles, which render upright
(evidence-2.png). That half is **not a missing property**: `i` is parsed and it reaches font
selection intact. It is the same font-fallback defect already written up as
`text-run-props-bold-ignored`, and fixing that issue fixes these four runs with no extra work.
Only the superscript half needs new code.

Seen on 2 slides across 2 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact low, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/05** `TextBox 8` footnote marker, `sz="1200" baseline="30000"`: raised and reduced in the reference, full-size on the baseline in the candidate

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-misc-property-ignored/evidence-1.png)

**2. ocp-psp-plan/02** The `i="1"` role subtitles under two Incentives-pod names render upright; the bold name above them is likewise unbolded, which is the tell that both come from the same fallback face

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-misc-property-ignored/evidence-2.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 5
- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 2

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(4, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

### Superscript: `baseline` is never parsed

`parse_run_properties` ([`crates/pptx-parse/src/drawing.rs:900`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L900)) reads `sz`, `b`, `i`, `u`,
`a:latin/@typeface`, `a:solidFill`, `lang` and `a:hlinkClick/@r:id`, and nothing else.
`RunProperties` ([`crates/pptx-parse/src/model.rs:338`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L338)) has no field for a baseline shift, so the
attribute is dropped at the XML boundary and cannot reach layout. Downstream there is nothing to
receive it either: `ResolvedStyle` ([`crates/pptx-render/src/layout.rs:924`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L924)) carries only face,
family, size, bold, italic, underline and colour, and `PositionedTextRun`
([`crates/pptx-render/src/display_list.rs:230`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L230)) is the same list plus geometry.

Confirmed against the deck. The run is
`<a:rPr lang="en-US" sz="1200" baseline="30000" dirty="0">` in
`render-improvement-harness/decks/cisco-cloud-security/xml/05/slide.xml`, and the display list
for that slide merges the marker into its neighbour as one run:

```
{'text': '75% of mobile apps fail basic security tests1', 'fontSizePx': 16.0, ...}
  lineBaseline=452.89  glyphY=[452.89, 452.89, 452.89]
```

The marker has the same `fontSizePx` and the same glyph `y` as the sentence — no size change, no
shift, and not even a run boundary, because `positioned_runs`
([`crates/pptx-render/src/layout.rs:1472`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1472)) coalesces adjacent clusters on `(end == start, font_id)`
alone, and a baseline shift is not part of that key.

Measured off the two 96 dpi images (`decks/cisco-cloud-security/{lo,bo}-img/05.png`, red-ink
column runs in the caption band): the base digits ink 12 rows tall in the reference, the marker
inks 6 rows, its baseline sits ~5px above the text baseline. 5px is 31% of the 16px em, which is
exactly `baseline="30000"`. The candidate's marker inks 11 rows on the shared baseline. The raise
therefore comes straight from the attribute; the size reduction to roughly half does not, and is
renderer policy — LibreOffice's superscript default is 58% of the run size.

Not confirmed: what PowerPoint's reduction ratio is. The repo's own docx convention is 0.75 with a
0.4em raise ([`crates/ooxml-text/src/measure/prepare.rs:466`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/measure/prepare.rs#L466)), which is larger than what the
reference draws here. Note that pptx-render does not use the `ooxml-text` measure pipeline at all
— it imports only `shape`, `break_opportunities` and `single_line_box`
([`crates/pptx-render/src/layout.rs:8`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L8)) — so the docx superscript support is not reusable, only its
convention is.

`baseline="0"` also appears on `defRPr` in the layout's `lstStyle`
(`decks/cisco-cloud-security/xml/05/slide.xml`), and `parse_run_properties` serves `defRPr` too,
so once modeled the cascade picks it up for free and a `0` is a no-op shift.

### Italic: a duplicate of `text-run-props-bold-ignored`

`i="1"` is parsed ([`crates/pptx-parse/src/drawing.rs:911`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L911)), stored
([`crates/pptx-parse/src/model.rs:341`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L341)), carried through the snapshot
([`crates/pptx-edit/src/story.rs:645`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L645), [`crates/pptx-edit/src/model.rs:38`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L38)), resolved
([`crates/pptx-render/src/layout.rs:1020`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1020)) and passed to `resolve_face`
([`crates/pptx-render/src/layout.rs:1041`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1041)). It survives all the way to the display list — the run
for "Incentives/Investment " comes out with `italic: True`.

It is thrown away in `resolve_face` ([`crates/pptx-render/src/layout.rs:245`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L245)):

```rust
self.faces
    .get(&(normalized.clone(), bold, italic))                // 253
    .or_else(|| self.faces.get(&(normalized, false, false)))  // 254
    .or(self.fallback.as_ref())                              // 255
```

These runs ask for `Segoe UI`, which no host registers, so both lookups miss and the run lands on
`self.fallback` — the first face ever registered ([`crates/pptx-render/src/layout.rs:111`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L111)), which
carries no style. Nothing downstream can recover: there is no synthetic oblique anywhere in
`crates/pptx-raster/src/font.rs`, which only fills the outlines of the face it is handed.

Confirmed by experiment. Rendering ocp-psp-plan slide 2 twice through `bindings/python-pptx`,
changing nothing but which Liberation Sans face is registered first, the italic run resolves to
`fontId 0` both times — that is, to whichever face happened to be registered first:

```
regular-first face ids per style: {'Regular': 2, 'Bold': 5, 'Italic': 8, 'BoldItalic': 11}
  -> italic run resolves to fontId 0   (= the Arial Regular registered first)
italic-first  face ids per style: {'Italic': 2, 'Regular': 5, 'Bold': 8, 'BoldItalic': 11}
  -> italic run resolves to fontId 0   (= the Arial Italic registered first)
```

This is the defect `text-run-props-bold-ignored` describes, and its proposed fix — a
style-aware fallback chain that degrades family before style — resolves `(fallback family,
false, true)` and gives these runs Liberation Sans Italic. No separate work is needed.

The browser backend does not share the defect for either flag:
[`packages/pptx/src/render/canvas.ts:233`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L233) builds its CSS font from `run.italic`, so the canvas
already slants text the raster leaves upright. Fixing `resolve_face` removes that divergence too.

**Suggested fix**

Two halves, one of which is somebody else's change.

**Italic — do nothing here.** Take `text-run-props-bold-ignored`'s style-aware fallback chain in
`resolve_face` ([`crates/pptx-render/src/layout.rs:245`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L245)). Its step "the same four style keys against
`fallback_family`" resolves `(fallback family, false, true)` and hands these runs Liberation Sans
Italic. Adding a second, italic-only fix would collide with it.

**Superscript — thread one `Option<f64>` from the XML to the glyph `y`.** The value is a
percentage of the font size, positive for superscript and negative for subscript, so one field
covers both. It changes two things at layout time: the run shapes at a reduced size, and its
glyphs are offset off the line's baseline.

1. **Parse.** Add `baseline_pct: Option<f64>` to `RunProperties`
   ([`crates/pptx-parse/src/model.rs:338`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L338)) and read `baseline` in `parse_run_properties`
   ([`crates/pptx-parse/src/drawing.rs:900`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L900)), dividing by 1000 like `sz` divides by 100, and
   rejecting non-finite values. `parse_run_properties` also serves `defRPr`
   ([`crates/pptx-parse/src/drawing.rs:879`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L879)), so `lstStyle`/layout/master defaults come along free.
2. **Keep the write path honest.** Add the arms to `apply_run_properties`
   ([`crates/pptx-parse/src/write.rs:1520`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1520)) and `run_properties_element`
   ([`crates/pptx-parse/src/write.rs:1667`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1667)). `apply_run_properties` deletes every *modeled*
   attribute whose field is `None`, so this is not optional: `baseline` survives a round-trip
   today only because it is unmodeled. Its docstring at
   [`crates/pptx-parse/src/write.rs:1518`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1518) lists what the model does not carry and needs updating.
3. **Carry it through the snapshot.** Add the field to `TextStyle`
   ([`crates/pptx-edit/src/model.rs:36`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L36)) and `TextStylePatch`
   ([`crates/pptx-edit/src/model.rs:47`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L47)), and populate it in `style_from_run_properties`
   ([`crates/pptx-edit/src/story.rs:643`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L643)), `style_from_attrs`
   ([`crates/pptx-edit/src/story.rs:654`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L654), a Yjs `Any::Number`), `run_write`
   ([`crates/pptx-edit/src/save.rs:384`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L384)), `style_from_properties`
   ([`crates/pptx-render/src/layout.rs:1849`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1849)) and `merge_run_properties`
   ([`crates/pptx-render/src/layout.rs:1825`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1825)).
4. **Resolve into pixels.** Add `baseline_shift_px` (and an effective size) to `ResolvedStyle`
   ([`crates/pptx-render/src/layout.rs:924`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L924)), computed in `resolve_style`
   ([`crates/pptx-render/src/layout.rs:1010`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1010)) direct-then-fallback. Shrink the size there, once, so
   every consumer of `style.font_size_pt` — shaping, line box, the display list — agrees without
   knowing why the run is small.
5. **Offset the glyphs.** `positioned_runs` ([`crates/pptx-render/src/layout.rs:1472`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1472)) already
   writes `y_offset: baseline + glyph.y_offset` at `:1516`; subtract the run's shift there. Two
   companion edits: the coalescing test at [`crates/pptx-render/src/layout.rs:1484`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1484) must include the
   shift in its key, or the marker keeps merging into the sentence run (it does today); and
   `clusters_line_box` ([`crates/pptx-render/src/layout.rs:1525`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1525)) must fold the shift into ascent
   and descent the way [`crates/ooxml-text/src/measure/line_filler.rs:547`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/measure/line_filler.rs#L547) does, so a tall
   superscript grows the line instead of clipping.
6. **Tell the canvas.** `crates/pptx-raster/src/font.rs` needs no change — it paints the absolute
   `glyph.y_offset` it is handed. The browser backend does: it draws a whole run with one
   `fillText` at `line.baseline` ([`packages/pptx/src/render/canvas.ts:237`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L237)), so
   `PositionedTextRun` ([`crates/pptx-render/src/display_list.rs:230`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L230)) needs a
   `baselineOffsetPx` that `paintTextRun` subtracts. Without it raster and canvas diverge on
   exactly these runs. `underline` is drawn from `line.baseline` in both backends
   ([`crates/pptx-raster/src/font.rs:110`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/font.rs#L110), [`packages/pptx/src/render/canvas.ts:239`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L239)) and should use
   the shifted baseline too.

```rust
// crates/pptx-parse/src/drawing.rs, in parse_run_properties
baseline_pct: element
    .attribute("baseline")
    .and_then(|value| value.parse::<f64>().ok())
    .filter(|value| value.is_finite() && value.abs() <= 100_000.0)
    .map(|value| value / 1000.0),

// crates/pptx-render/src/layout.rs, in resolve_style
let baseline_pct = direct
    .baseline_pct
    .or_else(|| fallback.and_then(|value| value.baseline_pct))
    .filter(|value| value.is_finite())
    .unwrap_or(0.0) as f32;
let scripted = baseline_pct != 0.0;
let font_size_pt = if scripted { font_size_pt * SCRIPT_SIZE_RATIO } else { font_size_pt };
let baseline_shift_px = points_to_px(font_size_pt) * baseline_pct / 100.0;

// crates/pptx-render/src/layout.rs, in positioned_runs
let append = output.last().is_some_and(|run| {
    run.end == cluster.start
        && run.font_id == cluster.style.face.id.to_u32()
        && run.baseline_offset_px == cluster.style.baseline_shift_px   // new
});
// ...
y_offset: baseline - cluster.style.baseline_shift_px + glyph.y_offset,
```

`SCRIPT_SIZE_RATIO` is the one judgement call. The reference's marker measures at roughly half the
base em (report, "Root cause"); LibreOffice's own default is 0.58; the repo's docx path uses 0.75
([`crates/ooxml-text/src/measure/prepare.rs:466`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/measure/prepare.rs#L466)). 0.58 matches the evidence in hand, but the
number that matters is PowerPoint's, which this investigation did not establish — pick it by
rendering a PowerPoint-authored superscript, not by reading.

Whether the shift is a percentage of the *original* or the *reduced* size is a second small
choice; the sketch uses the reduced size, and at 30000 the two differ by ~2px at 12pt.

Risks and tests to add:

- **Silent `baseline` loss on save.** `apply_run_properties` removes any modeled attribute left
  `None`. `baseline` round-trips safely today precisely because it is unmodeled; modeling it
  without step 2 deletes superscripts from user files. An edit-and-save round-trip test over
  cisco-cloud-security slide 5 is the guard.
- **Wrap points move.** A shrunk run advances less, so a line holding a superscript can gain
  characters. Only runs carrying a non-zero `baseline` are affected, and `baseline="0"` on
  `defRPr` must stay a strict no-op — assert that, or every deck with a default `lstStyle` shifts.
- **Line growth.** Folding the shift into ascent (step 5) makes a superscript-bearing line taller
  than its neighbours, which is correct but moves everything below it in a `spAutoFit` box. If
  ascent is left alone instead, a large `baseline` clips at the top of the text box.
- **Subscript is untested.** The same field carries `baseline="-25000"`, and neither deck in this
  cluster has one. Either implement both and test the negative case synthetically, or reject
  negatives explicitly rather than shipping an untried path.
- **Golden churn.** Existing pptx goldens have no `baseline`, so they must not move; that is the
  regression check. A superscript fixture added to `crates/pptx-raster/tests/golden.rs` pins the
  new behaviour.
- Tests to add: `baseline` parse and round-trip in `crates/pptx-parse` (extend the `rPr` fixture at
  [`crates/pptx-parse/src/drawing.rs:956`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L956)); run-size, glyph-`y` and non-coalescing assertions in the
  [`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2008) test module; a `baseline="0"` no-op case.

**How to verify**

Superscript: re-render and re-diff cisco-cloud-security.

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py cisco-cloud-security
.venv/bin/python render-improvement-harness/scripts/diff.py cisco-cloud-security
```

Slide 05's `fine_pct` is 4.52 and is dominated by the `custGeom` icons
(`geometry-custom-collapses-to-bbox`), so expect only a fraction of a percent from this change.
The real check is the ink: in `bo-img/05.png` the caption band's last red column run must ink
about 6 rows ending ~5px above the sentence baseline, matching `lo-img/05.png`, instead of the 11
rows on the shared baseline it inks today.

Italic: nothing to verify separately — it is covered by `text-run-props-bold-ignored`'s
verification on ocp-psp-plan/02.

No existing coverage for either. [`crates/pptx-parse/src/drawing.rs:956`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L956)
(`parses_text_formatting_and_nested_shape_types`) is the `rPr` fixture a `baseline` case belongs in; the `crates/pptx-render/src/layout.rs`
test module ([`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2008)) has nothing asserting run size or glyph `y`
for a shifted run; `crates/pptx-raster/tests/golden.rs` has no superscript fixture.

**Additional context**

none.

Related issues found in the same run: `geometry-custom-collapses-to-bbox`, #266

Files most likely involved: `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/write.rs`, `crates/pptx-edit/src/model.rs`, `crates/pptx-edit/src/story.rs`, `crates/pptx-edit/src/save.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/display_list.rs`, `packages/pptx/src/render/canvas.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-misc-property-ignored/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
