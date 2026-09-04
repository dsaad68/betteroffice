# pptx: Group child rotation + anisotropic scale computed wrong

**Describe the bug**

On `swot-analysis/01` the small shading crescent that hugs the rim of each SWOT icon renders as an
oversized diamond that pokes out past the circle, on all four icons (evidence-1.png,
evidence-2.png). The finding attributes this to the group's anisotropic `chOff`/`chExt` scale being
composed wrongly with the child's own `rot="18900000"`.

**That attribution is wrong.** Measured against the rendered pixels, BetterOffice's group child
transform is correct to within ~2px; the diamond is simply the *bounding rectangle* of the
`a:custGeom` freeform, correctly scaled and correctly rotated 315° (evidence-3.png). This is
`geometry-custom-collapses-to-bbox` seen through a 315° rotation, nothing more.

Seen on 1 slide across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact low, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. swot-analysis/01** The "S" icon at 2x: reference draws a thin rim-hugging crescent, candidate draws a straight-edged diamond — a rotated rectangle, not a mis-scaled freeform.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/transform-group-child-rotation-scale-wrong/evidence-1.png)

**2. swot-analysis/01** All four icons; the same single shape repeated four times in four sibling groups, so the cluster's one finding is really one shape.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/transform-group-child-rotation-scale-wrong/evidence-2.png)

**3. swot-analysis/01** The proof. Green = the freeform's real path, transformed by BetterOffice's own model, drawn over the reference: it traces the reference crescent exactly. Red = the same freeform's *bounding box* under the same transform, drawn over the candidate: it traces the candidate diamond exactly.

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/transform-group-child-rotation-scale-wrong/evidence-3.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `SWOT Analysis Slide Design Template in Microsoft PowerPoint 2016.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 1

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(0, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**Confirmed, and it is not a transform bug.** The transform hypothesis in the finding is
**not confirmed — it is refuted by measurement.**

### What the code does

`Space::for_group` ([`crates/pptx-render/src/layout.rs:1622`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1622)) builds an axis-aligned
scale-plus-translate from the group's `a:xfrm`: `scale_x = rect.w / chExt.cx`,
`scale_y = rect.h / chExt.cy` ([`crates/pptx-render/src/layout.rs:1628-1629`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1628-L1629)), origin shifted by
`chOff` ([`crates/pptx-render/src/layout.rs:1633-1634`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1633-L1634)). `Space::map_transform`
([`crates/pptx-render/src/layout.rs:1613`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1613)) maps a child's **unrotated** `a:off`/`a:ext` through it.
The child's own `rot` never touches that box: it is carried separately onto the primitive as
`Transform { rotation_deg, .. }` ([`crates/pptx-render/src/layout.rs:392-396`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L392-L396) on the snapshot path,
[`crates/pptx-render/src/layout.rs:500-504`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L500-L504) on the parsed path) and applied by the raster backend as
a rotation about the primitive box's centre ([`crates/pptx-raster/src/lib.rs:556-565`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L556-L565), composed at
[`crates/pptx-raster/src/lib.rs:241`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L241)).

That is exactly PowerPoint's model — scale the unrotated box by the group factors, then rotate the
result about its own centre; the anisotropic scale never shears the rotated shape.

### The measurement

For `Freeform: Shape 21` (id 22) in `Group 24` (id 25), with `sx = 2351315/4063689 = 0.578625`,
`sy = 2104570/3744686 = 0.562014`, child `off (4464751, 2391876)` `ext (3387166, 2336802)`,
`rot="18900000"` (315°), at 1280x720:

| corner | predicted (px) | measured in `bo-img/01.png` |
|---|---|---|
| right | (658.4, 240.8) | (657, 240) |
| top | (560.9, 143.3) | (560, 143) |
| bottom | (512.9, 386.3) | (512.5, 384) |

Rasterising the freeform's real `a:pathLst` through the *same* transform and comparing against the
reference: **100.0% of the reference crescent's pixels fall inside the predicted path** on the S, O
and W icons and 99.8% on the T icon. Everything inside the predicted path that the reference does
not paint in the crescent colour is accounted for — 15929px covered by the later `Oval 23`
highlight, 1253px by the white "S" glyph, the rest antialiasing. Conversely **100.0% of the
candidate's diamond pixels fall inside the freeform's bounding box under that same transform**, on
all four icons.

So both engines place the shape identically. The reference draws the authored outline; the
candidate draws its bounding rectangle.

### Why the rectangle

Same chain as `geometry-custom-collapses-to-bbox`: `parse_geometry`
([`crates/pptx-parse/src/drawing.rs:335`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L335)) reduces `<a:custGeom>` to the string `"custom"`
([`crates/pptx-parse/src/drawing.rs:340-343`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L340-L343)) and never reads `a:pathLst`; `geometry_path`
([`crates/pptx-render/src/layout.rs:1946`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1946)) asks `preset_geometry_to_path` for `"custom"`, gets `None`
([`crates/ooxml-drawingml/src/geometry.rs:227`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L227)), and falls back to `"rect"`
([`crates/pptx-render/src/layout.rs:1955-1956`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1955-L1956)).

The path here is a circle-segment cap — flat chord from `(314725,0)` to `(3072440,0)`, then two
`cubicBezTo` arcs down to `(1693583,2336802)` — occupying only ~40% of its 3387166x2336802 box. Fill
in the path and the diamond becomes the crescent.

### What this cluster adds over `geometry-custom-collapses-to-bbox`

- It is the corpus's **only** custGeom shape under a non-uniformly scaled group *and* a non-zero
  child rotation, which makes it the sharpest regression fixture for that fix: a wrong compose order
  (rotate-then-scale, i.e. shearing) would move the corners by tens of pixels and the measurement
  above would catch it. The 17 findings in the other cluster are all axis-aligned or uniformly
  scaled.
- It independently pins the transform contract described above, which the other report does not
  touch.

### Latent gap found while checking, *not* this finding's cause

`Space` ([`crates/pptx-render/src/layout.rs:1596-1601`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1596-L1601)) carries only origin and scale, so a group's
own `rot`/`flipH`/`flipV` on `<p:grpSpPr><a:xfrm>` is dropped for its children at both group sites
([`crates/pptx-render/src/layout.rs:367-368`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L367-L368), [`crates/pptx-render/src/layout.rs:492`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L492)). All four
groups on this slide have no `rot`, so it does not affect this cluster. Across the whole corpus only
2 of 929 group transforms carry `rot` (`cisco-cloud-security/07` `rot="20679101"`,
`cisco-cloud-security/09` `rot="476079"`) and neither produced a filed finding — **not confirmed as
a visible defect**, and out of scope here.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Do not fix anything in the transform code. The measurement in `report.md` shows
`Space::for_group` + the per-primitive `Transform` already reproduce the reference placement of
this shape to within ~2px on all four icons, so there is no transform defect to repair.

Two actions:

1. **Fold this cluster into `geometry-custom-collapses-to-bbox`.** Add `swot-analysis/01/1` to that
   cluster's `findings`, `swot-analysis` to its `decks`, and mark this issue `status: duplicate`.
   Parsing `a:pathLst` fixes it with no extra work.
2. **Keep the slide as a regression fixture for that fix.** It is the corpus's only custGeom shape
   that is simultaneously (a) inside a group whose `chExt` scale is anisotropic (sx 0.5786 vs
   sy 0.5620) and (b) carrying its own `rot`. It is therefore the one slide that would catch a
   custGeom implementation that scales in the wrong frame, or a future refactor that folds the
   child rotation into the group space matrix and starts shearing.

The useful artefact from this investigation is a regression test, not a patch.

```rust
// crates/pptx-render/src/layout.rs — new test beside the existing layout tests.
// Locks the compose order: scale the UNROTATED child box by the group's chExt
// factors, then rotate about the resulting box's centre. No shear.
#[test]
fn group_child_keeps_its_own_rotation_under_anisotropic_child_scale() {
    // grpSpPr: off (3743039,1190173) ext (2351315,2104570)
    //          chOff (3788228,1190173) chExt (4063689,3744686)
    // child:   rot 18900000, off (4464751,2391876) ext (3387166,2336802)
    let group = ShapeTransform { /* ... */ };
    let child = ShapeTransform { rotation_deg: 315.0, /* ... */ };

    let rect = Space::root().map_transform(&group);
    let child_rect = Space::for_group(rect, &group).map_transform(&child);

    // 3387166 * 0.578625 / EMU_PER_CSS_PIXEL, 2336802 * 0.562014 / ...
    assert!((child_rect.w - 205.76).abs() < 0.05);
    assert!((child_rect.h - 137.88).abs() < 0.05);
    // Anisotropy must survive: a shearing compose would equalise these.
    assert!((child_rect.w / child_rect.h - 1.492).abs() < 0.01);

    // Emitted primitive carries the child's rot, unmodified by the group.
    // corners of the rotated box: right (658.4,240.8), top (560.9,143.3)
}
```

A `crates/pptx-raster/tests/golden.rs` golden of the S icon (circle + rotated freeform + highlight
oval) is the stronger check once the custGeom path lands, because it catches the fill rule and the
z-order of the highlight oval at the same time.

Risks and tests to add:

Low — this proposes no production change. The only risk is procedural: if the fold is done by
deleting this issue rather than marking it `duplicate`, the measurement that refutes the original
"transform is wrong" hypothesis is lost and someone re-derives it from the same screenshot.

If the fix author does decide to touch group transforms, the one thing that must not change is that
the group's `chOff`/`chExt` scale is applied to the child's **unrotated** `a:off`/`a:ext`
([`crates/pptx-render/src/layout.rs:1613`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1613), [`crates/pptx-render/src/layout.rs:1622`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1622)) while the child's
`rot` stays on the primitive ([`crates/pptx-render/src/layout.rs:392-396`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L392-L396),
[`crates/pptx-render/src/layout.rs:500-504`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L500-L504)). Moving the rotation into `Space` would introduce the
shear that PowerPoint does not apply, and this slide would then regress in the opposite direction.

The separate, unconfirmed group-`rot`/`flipH`/`flipV` gap noted in `report.md` should be filed on its
own evidence if anyone wants it; it is not exercised by this deck.

**How to verify**

Nothing to fix independently. Fold `swot-analysis/01/1` into `geometry-custom-collapses-to-bbox` and
add `swot-analysis` to that cluster's verification set.

After the custGeom path parse lands, re-render with
`.venv/bin/python render-improvement-harness/scripts/render_bo.py swot-analysis` then
`diff.py swot-analysis`. Slide 01's `diff_pct` is 8.86; the four diamonds are the only geometric
difference on the slide (findings 2 and 3 are `lo-suspect` text wraps where LibreOffice is the wrong
one), so it should drop to roughly the text-wrap residue. Check the crescents sit *inside* the
circle rims and that the freeform's straight chord edge runs bottom-left to top-right at 45°, which
is what proves the rotation survived the fix.

Worth pinning as a test: no test under `crates/pptx-render` covers a rotated child inside an
anisotropically scaled group. `Space::for_group` has no direct unit test — the nearest coverage is
the emitted-primitive assertions around [`crates/pptx-render/src/layout.rs:2152-2154`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2152-L2154), and the raster
side's box-to-pixel contract at [`crates/pptx-raster/src/lib.rs:953`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L953)
(`geometry_commands_scale_by_the_primitive_box`). A layout test asserting the four rotated corners
of this exact shape would lock in the no-shear compose order.

**Additional context**

none.

Related issues found in the same run: `geometry-custom-collapses-to-bbox`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-render/src/layout.rs`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/transform-group-child-rotation-scale-wrong/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
