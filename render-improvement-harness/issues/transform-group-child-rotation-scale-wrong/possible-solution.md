# Possible solution: transform-group-child-rotation-scale-wrong

## Approach

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

## Sketch

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

## Risks

Low — this proposes no production change. The only risk is procedural: if the fold is done by
deleting this issue rather than marking it `duplicate`, the measurement that refutes the original
"transform is wrong" hypothesis is lost and someone re-derives it from the same screenshot.

If the fix author does decide to touch group transforms, the one thing that must not change is that
the group's `chOff`/`chExt` scale is applied to the child's **unrotated** `a:off`/`a:ext`
(`crates/pptx-render/src/layout.rs:1613`, `crates/pptx-render/src/layout.rs:1622`) while the child's
`rot` stays on the primitive (`crates/pptx-render/src/layout.rs:392-396`,
`crates/pptx-render/src/layout.rs:500-504`). Moving the rotation into `Space` would introduce the
shear that PowerPoint does not apply, and this slide would then regress in the opposite direction.

The separate, unconfirmed group-`rot`/`flipH`/`flipV` gap noted in `report.md` should be filed on its
own evidence if anyone wants it; it is not exercised by this deck.

## Effort

easy — no code change; add the finding to `geometry-custom-collapses-to-bbox` and, when that lands,
add the compose-order layout test above.
