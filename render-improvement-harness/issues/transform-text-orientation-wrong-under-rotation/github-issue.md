# pptx: Text orientation not corrected for shape rotation/flip

**Describe the bug**

Text inside a rotated or flipped shape comes out unreadable in two distinct ways.

The first: any shape whose `a:xfrm` carries an odd number of flips renders its text as a mirror
image. `rot="10800000" flipV="1"` (PowerPoint's encoding for "mirror horizontally") turns "Audit"
into "tibuA" and reverses "Panel C" (evidence-1.png); a bare `flipH="1"` does
the same to a whole paragraph (evidence-4.png); `rot="5400000" flipH="1"` rotates "Before"
correctly but runs the glyphs bottom-to-top and mirrored (evidence-3.png). LibreOffice and
PowerPoint rotate the text with the shape but never mirror the glyphs.

The second: a shape rotated 90°/270° that carries the compensating `<a:bodyPr vert="vert">` or
`vert="vert270"` ignores the `vert` attribute entirely. The text is laid out horizontally in the
shape's tall, narrow unrotated box — so it wraps to one word per line — and is then rotated with
the shape, producing a sideways column of single words instead of a horizontal sentence
(evidence-2.png).

Seen on 7 slides across 2 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/23** "Audit" and "Panel C" (`rot="10800000" flipV="1"`) drawn as a horizontal mirror image; the reference has them upright.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/transform-text-orientation-wrong-under-rotation/evidence-1.png)

**2. cisco-cloud-security/21** `rot="5400000"` + `vert="vert270"` label bars and `rot="16200000"` + `vert="vert"` digit badges: the bar geometry lands correctly but the text is a sideways column of one-word lines.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/transform-text-orientation-wrong-under-rotation/evidence-2.png)

**3. cisco-cloud-security/07** `rot="5400000" flipH="1"` side label "Before": reference reads top-to-bottom, candidate reads bottom-to-top and is mirrored.

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/transform-text-orientation-wrong-under-rotation/evidence-3.png)

**4. project17/05** a bare `flipH="1"` with no rotation: the whole "Partnership with Elixir…" paragraph is mirrored, right-aligned and unreadable.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/transform-text-orientation-wrong-under-rotation/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 7, 8, 9, 10, 21, 23
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 5

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(6, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

Confirmed. The text box primitive is given the *same* `Transform` as the shape, and the raster
and canvas backends both apply that transform — flips included — to the glyphs.

[`crates/pptx-render/src/layout.rs:394`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L394) (snapshot path) and [`crates/pptx-render/src/layout.rs:500`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L500)
(parsed path) build one `Transform { rotation_deg, flip_h, flip_v }` from the shape's `a:xfrm`,
and hand that exact value to `render_text_box` at [`crates/pptx-render/src/layout.rs:460`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L460) and
[`crates/pptx-render/src/layout.rs:565`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L565). `render_text_box` stores it unchanged on the
`Primitive::TextBox` it emits ([`crates/pptx-render/src/layout.rs:742`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L742), field at
[`crates/pptx-render/src/layout.rs:754`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L754)).

Both backends then turn `flip_h`/`flip_v` into a `-1` scale about the primitive's centre and
paint the text under it: [`crates/pptx-raster/src/lib.rs:242`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L242) composes the transform for every
primitive, the `Primitive::TextBox` arm at [`crates/pptx-raster/src/lib.rs:282`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L282) passes it into
`font::paint_lines`, and [`crates/pptx-raster/src/lib.rs:557`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L557) is where the flip becomes
`Transform::from_scale(-1.0, …)`. The canvas backend does the identical thing at
[`packages/pptx/src/render/canvas.ts:69`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L69) and [`packages/pptx/src/render/canvas.ts:113`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L113). A
reflection in the glyph transform is exactly the observed mirroring.

The correct rule, read off the two reference cases: text follows the shape's rotation but is
never mirrored, so the reflection must be cancelled by a *local* horizontal flip. Writing the
shape's linear part as `A = R(rot)·S(fx, fy)`, the text uses `A·S(-1, 1)`, which collapses to a
pure rotation of `rot + 180°·flip_v` with no flips at all:

| flips | text rotation | check against the deck |
|---|---|---|
| none | `rot` | — |
| `flipH` only | `rot` | slide 07 `rot=90 flipH` → 90°, reference reads top-to-bottom (evidence-3.png); project17/05 bare `flipH` → 0°, reference upright (evidence-4.png) |
| `flipV` only | `rot + 180` | slides 09/23 `rot=180 flipV` → 0°, reference upright (evidence-1.png) |
| both | `rot + 180` | geometry is already a pure rotation; text matches it |

For the second half, `a:bodyPr/@vert` *is* parsed — [`crates/pptx-parse/src/drawing.rs:779`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L779) stores
it as `TextBody::vertical` ([`crates/pptx-parse/src/model.rs:271`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L271)) — but nothing in `pptx-render`
ever reads it. `BodyCascade` ([`crates/pptx-render/src/layout.rs:769`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L769)) exposes `anchor`, `autofit`
and the four insets and no `vertical` accessor, and `render_text_box` derives its content box
straight from the unrotated shape rect at [`crates/pptx-render/src/layout.rs:673`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L673) with no
width/height swap. `grep -rn "vertical" crates/pptx-render crates/pptx-raster` returns only the
unrelated `vertical_shift` anchor local, so the value is parsed and dropped.

That explains slide 21 exactly: the label bars are `cx=639990 cy=7718032` EMU (tall and narrow)
with `rot="5400000"`. Laying the sentence out in that unrotated box forces one word per line;
rotating 90° then yields the observed vertical word column. Applying `vert270` would lay the text
out in the swapped 7718032 × 639990 box and rotate it −90°, which cancels the shape's +90° to
give the horizontal, single-line sentence the reference shows.

### The deferred `project17/05/5` finding belongs here

`clusters.json` defers `project17/05/5` as `lo-suspect`, on the theory that PowerPoint really does
mirror a shape's text when `flipH="1"` sits directly on the shape, making LibreOffice's upright
text the deviation. The Cisco deck rules that out. Slides 09/23 are PowerPoint-authored cards that
spell their horizontal mirror as `rot="10800000" flipV="1"`, which is the *same* orthogonal matrix
as a bare `flipH="1"`; the labels on them ("Audit", "Panel C") plainly have to
read normally in the source deck. A renderer that composes `rot` and the flips into one matrix —
as PowerPoint does — cannot mirror one spelling and not the other. So PowerPoint keeps text
upright for both, `project17/05/5` is the same bug, and I have folded it into this cluster
(evidence-4.png). Confidence on this specific re-classification is medium-high: it is an inference
from the deck's authoring intent, not from a PowerPoint render.

Not confirmed: the behaviour for a *bare* `flipV="1"` with `rot="0"` (the table above predicts
upside-down text). Nothing in either deck exercises it, so that row is an extrapolation from the
rule rather than an observation.

Also worth noting: `HitRegion` ([`crates/pptx-render/src/layout.rs:1641`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1641)) keeps a single transform
for both the shape rect and its text, and `HitRegion::local_point`
([`crates/pptx-render/src/layout.rs:1651`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1651)) un-flips caret hits with it. The unit test
`hit_testing_flipped_text_reads_the_mirrored_caret` ([`crates/pptx-render/src/layout.rs:2070`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2070))
asserts that a `flip_h` shape reverses the caret mapping for its text — an expectation that
follows from this same bug.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Give the text box its own transform instead of reusing the shape's, and derive it inside
`render_text_box` ([`crates/pptx-render/src/layout.rs:657`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L657)) so both call sites — the snapshot path
at [`crates/pptx-render/src/layout.rs:460`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L460) and the parsed path at
[`crates/pptx-render/src/layout.rs:565`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L565) — are fixed at once. Nothing downstream needs to change:
`Primitive::TextBox` already carries a per-primitive `transform`, and `pptx-raster`
([`crates/pptx-raster/src/lib.rs:242`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L242)), the canvas backend
([`packages/pptx/src/render/canvas.ts:69`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L69)) and hit testing all read it from the primitive.

Two pieces:

1. **Un-mirror.** A shape transform with an odd number of flips is a reflection; text must not be
   reflected. Cancelling it with a local horizontal flip collapses to a pure rotation:
   `text_rotation = rot + if flip_v { 180 } else { 0 }`, with `flip_h`/`flip_v` cleared. Applies to
   all four flip combinations (see the table in `report.md`).

2. **`bodyPr/@vert`.** Add a `vertical()` accessor to `BodyCascade`
   ([`crates/pptx-render/src/layout.rs:769`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L769)) alongside `anchor()`, reading
   `TextBody::vertical` ([`crates/pptx-parse/src/model.rs:271`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L271), already parsed at
   [`crates/pptx-parse/src/drawing.rs:779`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L779)). For `vert` add +90° and for `vert270` add −90° to the
   text rotation, and lay the text out in the shape box with width and height swapped about the
   shape centre — the box the glyphs occupy before that extra quarter turn. Treat `horz` and the
   unhandled East-Asian values (`eaVert`, `mongolianVert`, `wordArtVert*`) as horizontal.

`HitRegion` ([`crates/pptx-render/src/layout.rs:1641`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1641)) needs the text transform and the swapped
text rect stored next to the shape's, so `local_point` ([`crates/pptx-render/src/layout.rs:1651`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1651))
maps caret hits through the transform the glyphs were actually painted with rather than the
shape's.

```rust
// layout.rs — inside render_text_box, replacing the passed-through `transform`

#[derive(Clone, Copy)]
enum TextFlow { Horizontal, Vert, Vert270 }

fn text_flow(vertical: Option<&str>) -> TextFlow {
    match vertical {
        Some("vert") => TextFlow::Vert,
        Some("vert270") => TextFlow::Vert270,
        _ => TextFlow::Horizontal,
    }
}

let flow = text_flow(cascade.vertical());
// glyphs are rotated with the shape but never mirrored: fold the reflection away
let mut text_rotation = transform.rotation_deg + if transform.flip_v { 180.0 } else { 0.0 };
text_rotation += match flow {
    TextFlow::Vert => 90.0,
    TextFlow::Vert270 => -90.0,
    TextFlow::Horizontal => 0.0,
};
let text_transform = Transform {
    rotation_deg: text_rotation.rem_euclid(360.0),
    flip_h: false,
    flip_v: false,
};

// vertical flow lays out in the shape box turned on its side, about the same centre
let text_rect = match flow {
    TextFlow::Horizontal => rect,
    _ => PxRect {
        x: rect.x + (rect.w - rect.h) / 2.0,
        y: rect.y + (rect.h - rect.w) / 2.0,
        w: rect.h,
        h: rect.w,
    },
};
// ... content_rect, layout_content, anchor shift and Primitive::TextBox all use
// `text_rect` and `text_transform` from here on; TextHit carries them out for HitRegion.
```

Risks and tests to add:

- `hit_testing_flipped_text_reads_the_mirrored_caret`
  ([`crates/pptx-render/src/layout.rs:2070`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2070)) asserts the current, wrong behaviour and must be
  rewritten: a flipped shape no longer reverses its text's caret mapping. The test builds its
  `HitRegion` by hand, so it will keep compiling and silently keep asserting the bug unless it is
  updated deliberately.
- Shape hit testing must keep using the shape transform. Splitting the two transforms on
  `HitRegion` is the part most likely to introduce a regression; the rect-membership test
  `hit_testing_a_rotated_shape_follows_its_painted_frame`
  ([`crates/pptx-render/src/layout.rs:2026`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2026)) guards it.
- The swapped text rect changes wrapping and autofit input for every `vert` shape, so
  `normAutofit`/`spAutoFit` shrink loops ([`crates/pptx-render/src/layout.rs:687`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L687)) now run against
  the rotated extent. That is correct, but it moves text on any deck that previously overflowed.
- Insets (`lIns`/`tIns`/`rIns`/`bIns`) are expressed in the text body's own frame, so under `vert`
  they rotate with it. The sketch applies them after the swap, which is the intended reading;
  worth an explicit test since slide 21's bars use asymmetric insets
  (`lIns="91440" tIns="45720" rIns="91440" bIns="91440"`).
- Charts (`Primitive::Chart`) and placeholders keep the shape transform — only `TextBox` changes.

Tests to add: a `pptx-render` unit test pinning `text rotation = rot + 180·flip_v` across the four
flip combinations; a `pptx-raster` golden with a rotated-and-flipped text box next to
`golden_rotated` ([`crates/pptx-raster/tests/golden.rs:306`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/tests/golden.rs#L306)); and a layout test that a `vert270`
shape with `cx << cy` lays its text out on one line.

**How to verify**

- Re-render `cisco-cloud-security` slides 07, 08, 09, 10, 21, 23 and `project17` slide 05 with
  `.venv/bin/python render-improvement-harness/scripts/pipeline.py` (or `render_bo.py` + `diff.py`)
  and read the mirrored labels in `bo-img/23.png` and `bo-img/09.png` directly. Slide 23's diff
  (currently 8.5%) and slide 21's (currently 4.0%) should drop; 21 is the cleaner signal because
  its other findings are small, while 07/08/09/10 still carry unrelated `grpFill`, `srcRect` and
  `pattFill` failures that dominate their diffs. `project17/05` is the check for the bare-flip case.
- Slide 21 additionally proves the layout half: the label-bar sentences must land on one line, not
  as a column of words.
- Existing coverage to extend: `golden_rotated` ([`crates/pptx-raster/tests/golden.rs:306`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/tests/golden.rs#L306)) rotates
  a shape but has no text, so it does not regress; add a golden with a rotated *and* flipped text
  box. `hit_testing_a_rotated_shape_follows_its_painted_frame`
  ([`crates/pptx-render/src/layout.rs:2026`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2026)) stays valid, but
  `hit_testing_flipped_text_reads_the_mirrored_caret` ([`crates/pptx-render/src/layout.rs:2070`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2070))
  encodes the buggy expectation and has to be updated once the text region carries its own
  transform.

**Additional context**

none.

Related issues found in the same run: none.

Files most likely involved: `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/display_list.rs`, `crates/pptx-raster/tests/golden.rs`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/transform-text-orientation-wrong-under-rotation/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
