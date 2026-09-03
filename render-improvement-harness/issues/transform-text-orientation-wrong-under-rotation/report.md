---
id: transform-text-orientation-wrong-under-rotation
title: Keep shape text upright under flips and honour bodyPr/@vert
category: transform
impact: medium
effort: medium
confidence: high
status: open
occurrences: 7
decks: [cisco-cloud-security, project17]
findings: [cisco-cloud-security/07/4, cisco-cloud-security/08/3, cisco-cloud-security/09/2, cisco-cloud-security/10/1, cisco-cloud-security/21/1, cisco-cloud-security/23/1, project17/05/5]
files: [crates/pptx-render/src/layout.rs, crates/pptx-render/src/display_list.rs, crates/pptx-raster/tests/golden.rs]
---

## Symptom

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

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/23 | "Audit" and "Panel C" (`rot="10800000" flipV="1"`) drawn as a horizontal mirror image; the reference has them upright. |
| 2 | cisco-cloud-security/21 | `rot="5400000"` + `vert="vert270"` label bars and `rot="16200000"` + `vert="vert"` digit badges: the bar geometry lands correctly but the text is a sideways column of one-word lines. |
| 3 | cisco-cloud-security/07 | `rot="5400000" flipH="1"` side label "Before": reference reads top-to-bottom, candidate reads bottom-to-top and is mirrored. |
| 4 | project17/05 | a bare `flipH="1"` with no rotation: the whole "Partnership with Elixir…" paragraph is mirrored, right-aligned and unreadable. |

Slides 08, 09 and 10 (`cisco-cloud-security/08/3`, `/09/2`, `/10/1`) are the same
`rot="10800000" flipV="1"` case as evidence-1.png and are not re-shown.

## Root cause (hypothesis)

Confirmed. The text box primitive is given the *same* `Transform` as the shape, and the raster
and canvas backends both apply that transform — flips included — to the glyphs.

`crates/pptx-render/src/layout.rs:394` (snapshot path) and `crates/pptx-render/src/layout.rs:500`
(parsed path) build one `Transform { rotation_deg, flip_h, flip_v }` from the shape's `a:xfrm`,
and hand that exact value to `render_text_box` at `crates/pptx-render/src/layout.rs:460` and
`crates/pptx-render/src/layout.rs:565`. `render_text_box` stores it unchanged on the
`Primitive::TextBox` it emits (`crates/pptx-render/src/layout.rs:742`, field at
`crates/pptx-render/src/layout.rs:754`).

Both backends then turn `flip_h`/`flip_v` into a `-1` scale about the primitive's centre and
paint the text under it: `crates/pptx-raster/src/lib.rs:242` composes the transform for every
primitive, the `Primitive::TextBox` arm at `crates/pptx-raster/src/lib.rs:282` passes it into
`font::paint_lines`, and `crates/pptx-raster/src/lib.rs:557` is where the flip becomes
`Transform::from_scale(-1.0, …)`. The canvas backend does the identical thing at
`packages/pptx/src/render/canvas.ts:69` and `packages/pptx/src/render/canvas.ts:113`. A
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

For the second half, `a:bodyPr/@vert` *is* parsed — `crates/pptx-parse/src/drawing.rs:779` stores
it as `TextBody::vertical` (`crates/pptx-parse/src/model.rs:271`) — but nothing in `pptx-render`
ever reads it. `BodyCascade` (`crates/pptx-render/src/layout.rs:769`) exposes `anchor`, `autofit`
and the four insets and no `vertical` accessor, and `render_text_box` derives its content box
straight from the unrotated shape rect at `crates/pptx-render/src/layout.rs:673` with no
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

Also worth noting: `HitRegion` (`crates/pptx-render/src/layout.rs:1641`) keeps a single transform
for both the shape rect and its text, and `HitRegion::local_point`
(`crates/pptx-render/src/layout.rs:1651`) un-flips caret hits with it. The unit test
`hit_testing_flipped_text_reads_the_mirrored_caret` (`crates/pptx-render/src/layout.rs:2070`)
asserts that a `flip_h` shape reverses the caret mapping for its text — an expectation that
follows from this same bug.

## Verification

- Re-render `cisco-cloud-security` slides 07, 08, 09, 10, 21, 23 and `project17` slide 05 with
  `.venv/bin/python render-improvement-harness/scripts/pipeline.py` (or `render_bo.py` + `diff.py`)
  and read the mirrored labels in `bo-img/23.png` and `bo-img/09.png` directly. Slide 23's diff
  (currently 8.5%) and slide 21's (currently 4.0%) should drop; 21 is the cleaner signal because
  its other findings are small, while 07/08/09/10 still carry unrelated `grpFill`, `srcRect` and
  `pattFill` failures that dominate their diffs. `project17/05` is the check for the bare-flip case.
- Slide 21 additionally proves the layout half: the label-bar sentences must land on one line, not
  as a column of words.
- Existing coverage to extend: `golden_rotated` (`crates/pptx-raster/tests/golden.rs:306`) rotates
  a shape but has no text, so it does not regress; add a golden with a rotated *and* flipped text
  box. `hit_testing_a_rotated_shape_follows_its_painted_frame`
  (`crates/pptx-render/src/layout.rs:2026`) stays valid, but
  `hit_testing_flipped_text_reads_the_mirrored_caret` (`crates/pptx-render/src/layout.rs:2070`)
  encodes the buggy expectation and has to be updated once the text region carries its own
  transform.
