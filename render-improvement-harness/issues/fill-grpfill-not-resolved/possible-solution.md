# Possible solution: fill-grpfill-not-resolved

## Approach

Resolve the inheritance in `pptx-parse`, where both halves of the information are already in
hand, so nothing downstream (render, raster, edit, react) has to learn about groups.

1. Give `parse_fill` a `grpFill` arm that returns a sentinel `ShapeFill::named("group")`
   (`crates/pptx-parse/src/drawing.rs:565`). This distinguishes "defer to my group" from
   "no fill element at all", which today both produce `None`.
2. In `parse_group` (`crates/pptx-parse/src/drawing.rs:252`), after the children are parsed,
   read the group's own fill from `grpSpPr` with the same `parse_fill`. If it is a concrete
   fill (not the sentinel), walk the whole child subtree and replace every remaining sentinel
   with a clone of it.

Because `parse_group` recurses bottom-up, this handles nesting for free: an inner group with
its own `solidFill` resolves its children first, so the outer pass finds no sentinels left
there; an inner group that itself carries `grpFill` (12 such groups in the harness decks)
leaves its children's sentinels in place for the outer pass to fill in. That covers the 25
two-level and 2 three-level cases measured in the decks.

3. In `common_slide_data` (`crates/pptx-parse/src/drawing.rs:55`-`59`), resolve one last time
   against the `spTree`'s own `grpSpPr` fill, then clear any sentinel that is still
   unresolved back to `None`, so the sentinel never escapes the crate. Without that sweep a
   `"group"` fill type could reach `paint` (`crates/pptx-render/src/layout.rs:1897`, harmless
   -- it resolves to `None`) and `fill_element` (`crates/pptx-parse/src/write.rs:1042`, which
   would raise `unsupported fill type`).

Round-tripping is safe: `save.rs` only emits a fill patch when `shape.fill != base.fill`
(`crates/pptx-edit/src/save.rs:204`), and the baseline is parsed through the same resolution,
so an untouched `grpFill` shape produces no patch and its `<a:grpFill/>` stays in the XML.

The alternative -- add `fill: Option<ShapeFill>` to `GroupShape`, seed it in
`crates/pptx-edit/src/deck.rs:162`, and thread an inherited fill down both group recursions
(`crates/pptx-render/src/layout.rs:490` and `:362`) -- keeps the model faithful to the XML
but touches four crates and the snapshot schema for the same pixels. Prefer it only if the
editor later needs to show or edit a group's fill.

## Sketch

```rust
// drawing.rs
const GROUP_FILL: &str = "group";

fn parse_fill(element: &XmlElement) -> Option<ShapeFill> {
    // ... existing noFill / solidFill / gradFill / blipFill arms ...
    if element.child("grpFill").is_some() {
        return Some(ShapeFill::named(GROUP_FILL));
    }
    None
}

fn resolve_group_fill(nodes: &mut [ShapeNode], fill: &ShapeFill) {
    for node in nodes {
        match node {
            ShapeNode::Shape(shape) => replace_sentinel(&mut shape.fill, fill),
            ShapeNode::Picture(picture) => replace_sentinel(&mut picture.fill, fill),
            // descend: an inner group that had no concrete fill left its children deferring
            ShapeNode::Group(group) => resolve_group_fill(&mut group.children, fill),
            ShapeNode::GraphicFrame(_) => {}
        }
    }
}

fn replace_sentinel(slot: &mut Option<ShapeFill>, fill: &ShapeFill) {
    if slot.as_ref().is_some_and(|f| f.fill_type == GROUP_FILL) {
        *slot = Some(fill.clone());
    }
}

// in parse_group, after children are parsed
let mut children = parse_shape_children(element, relationships, part, budget)?;
if let Some(fill) = element.child("grpSpPr").and_then(parse_fill)
    && fill.fill_type != GROUP_FILL
{
    resolve_group_fill(&mut children, &fill);
}

// in common_slide_data, after the top-level tree is parsed: resolve against the
// spTree's own grpSpPr fill, then clear leftovers so the sentinel never escapes.
```

## Risks

- **Custom geometry interaction.** Six of the sixteen findings are `custGeom` icons that are
  invisible today only because they have no fill. `geometry_path` falls back to the `rect`
  preset for `"custom"` (`crates/pptx-render/src/layout.rs:1955`-`1956`), so resolving their
  fill turns each icon into a solid coloured rectangle -- visually a regression on
  `cisco-cloud-security/05` and `project20/02, 06, 07, 08, 09` until
  `geometry-custom-collapses-to-bbox` lands. Land the two together, or land this one and
  accept the temporary diff on those slides.
- **Scheme colours on the group.** Several groups inherit via `<a:schemeClr val="bg1"/>` /
  `accent1` rather than an sRGB literal. The cloned `ColorValue` is resolved by the same
  `paint`/theme path the child would have used, so `theme-color-scheme-color-resolution-broken`
  applies here too -- do not read a wrong colour after this fix as a grpFill bug.
- **Non-solid group fills.** Cloning a gradient or picture fill onto every child is not what
  PowerPoint does (the group's fill is painted once across the group's bounds and children
  window into it). No such case exists in the harness decks -- all 251 resolve to `solidFill`
  -- but restricting the clone to `solid` (and leaving the sentinel otherwise) would keep the
  approximation honest.
- **Connectors stay broken.** 36 of the 299 `grpFill`s sit on `p:cxnSp`, which
  `parse_shape_children` (`crates/pptx-parse/src/drawing.rs:110`) never turns into a
  `ShapeNode`. Do not expect those to appear.

Tests to add: a `parse_fill`/`parse_group` unit test in
`crates/pptx-parse/src/drawing.rs` (module at `:951`) covering a two-level chain -- outer
group `solidFill`, inner group `grpFill`, leaf `grpFill` -- asserting the leaf resolves to
the outer colour; and a `pptx-render` layout test asserting the emitted `Primitive::Shape`
carries the group's `Paint::Solid`, alongside the existing fill assertions near
`crates/pptx-render/src/layout.rs:2334`.

## Effort

Easy. Roughly 40 lines confined to `crates/pptx-parse/src/drawing.rs` plus two tests; no
model, snapshot or TypeScript change, and the write path already tolerates the element.
