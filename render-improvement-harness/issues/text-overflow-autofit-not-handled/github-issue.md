# pptx: Text overflow/autofit semantics not implemented

**Describe the bug**

A text box whose text is taller than its box is hard-clipped at the box's bottom edge, so the
overflowing line is sliced through the middle of the glyphs (evidence-1.png) or lost entirely
(evidence-4.png). PowerPoint and LibreOffice let that text spill outside the box. Where the
inheritance chain does carry `<a:spAutoFit/>`, the opposite happens: BetterOffice shrinks the
font in 10% steps until the text fits, so one placeholder renders at 59% of its sibling's size
(evidence-2.png) and a 48pt title collapses to ~31pt (evidence-3.png) — `spAutoFit` never
changes the font size.

Twelve of the thirteen findings are these two behaviours. The thirteenth,
`cisco-cloud-security/01/1`, is **not** an autofit failure — see "Root cause".

Seen on 13 slides across 5 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/09** Title `<a:bodyPr/>` with no autofit anywhere in the chain: reference spills "business" below the 0.8in box, candidate slices it at the box edge and also starts the first line ~14px lower.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-overflow-autofit-not-handled/evidence-1.png)

**2. rollout-plan/06** Two sibling `idx=10`/`idx=11` placeholders, both `<a:spAutoFit/>`, both `sz="3529"`. The one whose text fits renders correctly; the one that overflows is shrunk to 0.9^5 = 59%. Reference draws both at the same size and lets the second spill below the green bar.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-overflow-autofit-not-handled/evidence-2.png)

**3. project17/03** `<a:bodyPr ...><a:spAutoFit/></a:bodyPr>` with an explicit `sz="4800"` run. Reference keeps 48pt and lets the text grow past the stored box; candidate rewraps it at ~31pt. (The face difference is a separate font-substitution issue.)

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-overflow-autofit-not-handled/evidence-3.png)

**4. project20/01** `<a:bodyPr lIns="91440" anchor="t"><a:noAutofit/></a:bodyPr>`: "Strategy, and Execution Plan" and "May 8, 2018" are clipped away entirely.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-overflow-autofit-not-handled/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 2, 9, 12, 13, 19, 20
- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 1
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 3, 8, 11
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 1
- `rollout-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 6

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(2, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

Three separate defects sit behind this cluster. All three are confirmed against the decks' XML.

**1. The rasteriser clips every text box to its own rect — confirmed.**
[`crates/pptx-raster/src/lib.rs:282`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L282) intersects the current clip with the primitive's `x/y/w/h`
([`crates/pptx-raster/src/lib.rs:288`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L288), helper at [`crates/pptx-raster/src/lib.rs:325`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L325)) and hands
that mask to `font::paint_lines`. There is no condition on it. DrawingML has no clip on autoshape
text: text that does not fit is drawn outside the shape. The layout stage already knows this —
it computes `overflow` at [`crates/pptx-render/src/layout.rs:739`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L739) and ships it on the primitive
([`crates/pptx-render/src/display_list.rs:130`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L130)) — but nothing in the repo reads that field. The
web canvas renderer clips the same way at [`packages/pptx/src/render/canvas.ts:218-220`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L218-L220).

This explains cisco-cloud-security 02/2, 09/1, 12/1, 13/1, 19/3, 20/2 (master title `bodyPr` with
no autofit child, `cy="731837"`), ocp-psp-plan/01/3, project17/08/2 and 11/3 (layout title has
`<a:noAutofit/>`, `cy="369332"`), and project20/01/3 (`<a:noAutofit/>` on the slide).

**2. `spAutoFit` is treated as shrink-to-fit — confirmed.**
[`crates/pptx-render/src/layout.rs:687-689`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L687-L689) enters the shrink loop for
`Some(TextAutofit::Normal { .. } | TextAutofit::Shape)`, and the loop at
[`crates/pptx-render/src/layout.rs:691-697`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L691-L697) multiplies the scale by 0.9 until the text fits or the
scale hits `MIN_AUTOFIT_SCALE = 0.5` ([`crates/pptx-render/src/layout.rs:28`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L28)). `spAutoFit` means
the *shape* resizes to the text; the font size is untouched. `TextAutofit::Shape` is parsed
correctly at [`crates/pptx-parse/src/drawing.rs:795-797`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L795-L797) into the variant at
[`crates/pptx-parse/src/model.rs:286-293`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L286-L293), so the value reaches layout intact — it is used wrongly,
not lost.

This explains rollout-plan/06/2 (0.9^5 = 0.590, the reported 59% exactly) and project17/03/1
(0.9^4 = 0.656, 48pt -> 31.5pt, the reported ~31pt).

**3. The anchor shift is clamped at zero — confirmed, contributing.**
[`crates/pptx-render/src/layout.rs:712-713`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L712-L713) clamps the centre/bottom shift with `.max(0.0)`, so
overflowing text in an `anchor="ctr"` box starts at the box top instead of spilling equally above
and below. That is the ~14px downward shift of the first line reported in cisco-cloud-security/19/3
and visible in evidence-1.png. It only matters once the clip is lifted, but it must be fixed with
it or centred titles will land in the wrong place.

**Not this cluster: `cisco-cloud-security/01/1`.** The finding claims a 60% shrink on the `ctrTitle`.
Checked in `decks/cisco-cloud-security/xml/01`: neither the slide's `<a:bodyPr/>`, the layout's
`<a:bodyPr anchor="b"/>` on the `ctrTitle` shape, nor the master's `title` placeholder `bodyPr`
carries any autofit child, so `BodyCascade::autofit` ([`crates/pptx-render/src/layout.rs:777-782`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L777-L782))
returns `None`, the scale stays 1.0 and the shrink loop never runs. The size drop is
5200 -> 3200 = 61.5%: the layout shape's `<a:lstStyle>` `defRPr sz="5200"` is ignored and the
master's `titleStyle` `sz="3200"` wins. That is `text-inheritance-layout-lststyle-ignored`, and
this issue's fix will not move that slide.

**Not investigated:** `lnSpcReduction` is parsed ([`crates/pptx-parse/src/drawing.rs:802`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L802)) and
stored ([`crates/pptx-parse/src/model.rs:291`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L291)) but never read by layout. No finding in this cluster
depends on it; it is a latent gap in the same code path, not a claim about these slides.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Three changes, independent enough to land separately.

**1. Never shrink for `spAutoFit`** — `render_text_box` in [`crates/pptx-render/src/layout.rs:687`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L687).
Drop `TextAutofit::Shape` from the `matches!` that guards the shrink loop, so only
`normAutofit` scales the font. `spAutoFit` then behaves like no autofit: the text is laid out at
its authored size and overflows. That is what LibreOffice draws on rollout-plan/06 — the green
placeholder bar keeps its stored height and the second line spills below it — so there is no need
to grow the shape rect as well. Growing the box would also change the shape's fill and outline,
which the reference does not do.

**2. Let text paint outside its box** — [`crates/pptx-raster/src/lib.rs:282-297`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L282-L297). Stop narrowing
the clip to the text box; pass the inherited `clip` (the group or chart clip, or `None`) straight
to `font::paint_lines`. Keep the off-surface cull, but compute it from the laid-out line extents
rather than the box, so a wholly off-slide text box still costs nothing. The `overflow` flag on
the primitive ([`crates/pptx-render/src/display_list.rs:130`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L130)) needs no new plumbing under this
approach; it stays a hint for the editor UI.

Mirror it in [`packages/pptx/src/render/canvas.ts:217-222`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L217-L222): drop the `beginPath`/`rect`/`clip`
from `paintTextBox`. Note `paintPrimitive` wraps each primitive in `ctx.save()`/`ctx.restore()`,
so removing the clip there does not leak state.

**3. Let a centred or bottom-anchored overflow spill symmetrically** —
[`crates/pptx-render/src/layout.rs:710-714`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L710-L714). Remove the `.max(0.0)` from the centre and bottom
shifts so an overflowing box centres its text on the box instead of pinning it to the top.

```rust
// layout.rs — 1. only normAutofit shrinks
if matches!(autofit, Some(TextAutofit::Normal { .. })) {
    while laid_out.total_height > content_rect.h && scale > MIN_AUTOFIT_SCALE { /* unchanged */ }
}

// layout.rs — 3. overflow spills both ways
let vertical_shift = match anchor {
    TextAnchor::Top => 0.0,
    TextAnchor::Center => (content_rect.h - laid_out.total_height) / 2.0,
    TextAnchor::Bottom => content_rect.h - laid_out.total_height,
};
```

```rust
// pptx-raster/src/lib.rs — 2. text is not clipped to its own box
Primitive::TextBox { lines, .. } => {
    if lines.is_empty() || !self.lines_on_surface(lines, transform) {
        return Ok(());
    }
    font::paint_lines(self.pixmap, self.resources, self.glyphs, lines, transform, clip)
}
```

Risks and tests to add:

- Overflowing text now paints over whatever sits below the box. That is correct, and it is what
  the reference does, but it will move pixels on slides the harness currently calls `match`, so
  every deck needs a re-render pass, not just the thirteen findings' slides.
- Change 1 makes `spAutoFit` boxes render larger text than before. On decks where the stored
  `cy` is stale relative to the text, more overflow appears — again matching the reference.
- Change 3 shifts every centred text box whose text overflows, including ones no finding names.
- `crates/pptx-raster/tests/golden.rs` `text.png` will need regenerating if its fixture overflows;
  [`packages/pptx/src/render/canvas.test.ts:113`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.test.ts#L113),205` assert that `clip` is called and must be
  updated to assert it is *not* called for a text box (the chart clip assertion stays).
- Add: a layout unit test next to
  `normal_autofit_scales_text_until_the_shape_height_is_respected`
  ([`crates/pptx-render/src/layout.rs:2239`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2239)) that sets `TextAutofit::Shape` and asserts the font
  size is unchanged and `overflow` is true; a second asserting a centred overflowing box places
  its first line above `rect.y`; and a raster golden with a text box taller than its shape.

**How to verify**

Re-render the thirteen findings' slides and compare `diff-summary.json`:

- Clip removal: `cisco-cloud-security` 02, 09, 12, 13, 19, 20; `project17` 08, 11; `project20` 01;
  `ocp-psp-plan` 01. The overflowing line must appear whole, below the box. Titles anchored `ctr`
  must also move up by roughly half the overflow.
- `spAutoFit`: `rollout-plan` 06 — both header placeholders must render at the same size, with
  "Monthly run exec meeting" wrapping to two lines that spill past the green bar.
  `project17` 03 — the title must stay at 48pt.
- `cisco-cloud-security` 01 must be unchanged by this fix.

Existing coverage: `normal_autofit_scales_text_until_the_shape_height_is_respected`
([`crates/pptx-render/src/layout.rs:2239`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2239)) is the only autofit test and must keep passing, since
`normAutofit` still shrinks. `crates/pptx-raster/tests/golden.rs` holds `text.png`; the
`a_clipped_primitive_off_the_surface_draws_nothing` test at [`crates/pptx-raster/src/lib.rs:899`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L899)
uses a `Chart` primitive, so lifting the text-box clip does not weaken it. Canvas coverage that
asserts the clip call is at [`packages/pptx/src/render/canvas.test.ts:113`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.test.ts#L113),205`.

**Additional context**

none.

Related issues found in the same run: `text-inheritance-layout-lststyle-ignored`

Files most likely involved: `crates/pptx-raster/src/lib.rs`, `crates/pptx-render/src/layout.rs`, `packages/pptx/src/render/canvas.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-overflow-autofit-not-handled/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
