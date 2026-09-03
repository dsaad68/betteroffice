# Possible solution: line-zero-extent-skipped

## Approach

Treat `p:cxnSp` as a `ShapeNode::Shape`. A connector is structurally an `p:sp` minus the `p:txBody`:
same `p:spPr` with `a:xfrm`, `a:prstGeom`, `a:ln`, only the non-visual wrapper differs
(`p:nvCxnSpPr` / `p:cNvCxnSpPr` instead of `p:nvSpPr` / `p:cNvSpPr`). Mapping it onto the existing
variant means nothing downstream has to change: `seed_shape`
(`crates/pptx-edit/src/deck.rs:123-138`), `ShapeSnapshot`, `render_snapshot_shape`
(`crates/pptx-render/src/layout.rs:340`), `render_parsed_shape`
(`crates/pptx-render/src/layout.rs:480`), the rasterizer and `packages/pptx/src/render/canvas.ts`
all already handle a `prstGeom` shape with an outline and no text.

Two edits:

1. `parse_shape_children` (`crates/pptx-parse/src/drawing.rs:109`) gains a `"cxnSp"` arm, and
   `parse_shape` (`crates/pptx-parse/src/drawing.rs:138-155`) looks for the non-visual wrapper under
   either name. `p:cxnSp` has no `p:txBody`, so `element.child("txBody")` already yields `None` and
   the text branch is a no-op.

2. `is_shape_element` (`crates/pptx-parse/src/write.rs:682-684`) must gain `"cxnSp"` in the same
   change. This is the load-bearing part: shape ids embed the parsed-shape ordinal
   (`seed_shape` builds `"{slide_id}:shape:{path}"`, `crates/pptx-edit/src/deck.rs:95`), `save.rs`
   recovers that ordinal (`source_index`, `crates/pptx-edit/src/save.rs:161-168`), and `write.rs`
   resolves it against the XML children filtered by `is_shape_element`
   (`crates/pptx-parse/src/write.rs:765-774`). Adding `cxnSp` to the parser but not to the writer
   desynchronises the two lists at the first connector on a slide, so every subsequent `Keep` or
   `Patch` would be applied to the wrong element - silent deck corruption on save, not a render bug.

## Sketch

```rust
// crates/pptx-parse/src/drawing.rs
let shape = match child.local_name() {
    "sp" | "cxnSp" => Some(ShapeNode::Shape(parse_shape(child, part, budget)?)),
    "pic" => ...,
};

fn parse_shape(element: &XmlElement, part: &str, budget: &mut ParseBudget<'_>) -> Result<Shape, PptxError> {
    budget.charge_shape(part)?;
    let properties = element.child("spPr");
    let transform = properties.and_then(|value| value.child("xfrm"));
    let non_visual = element.child("nvSpPr").or_else(|| element.child("nvCxnSpPr"));
    Ok(Shape {
        base: parse_base(non_visual, transform),
        // unchanged: geometry, adjust_values, fill, outline, text
    })
}
```

```rust
// crates/pptx-parse/src/write.rs
fn is_shape_element(local: &str) -> bool {
    matches!(local, "sp" | "cxnSp" | "pic" | "graphicFrame" | "grpSp")
}
```

## Risks

- **Save-path index alignment** (above). The two `matches!` lists are an undeclared invariant; the
  fix should land both sides together and ideally leave a note tying them. `cargo test -p pptx-edit
  --test write_fidelity` covers decks containing `p:cxnSp`
  (`crates/pptx-edit/tests/write_fidelity.rs:45`, `:70`, `:257`, `:615`) and is the guard here; add
  a case where a connector sits *between* two `p:sp` and a patch is applied to the later one, since
  a same-order deck would not catch a one-off shift.
- **New shape ids.** Any slide with a connector now exposes more shapes and renumbers the ones after
  it, so persisted shape ids from an earlier session no longer point at the same shape. Same class
  of change as any parser addition, but worth calling out to whoever owns the editing surface.
- **`shape_add` on a connector.** `crates/pptx-edit/src/save.rs:412-418` refuses anything that is
  not `ShapeKind::Shape`; a connector now *is* one, so a copy-then-add of a connector would be
  written back as a `p:sp`. Acceptable (it renders identically), but it silently changes the element
  kind on round trip. If that matters, carry a flag on `Shape` rather than widening the arm.
- **Incomplete win on other decks.** As noted in the report, 190 of 309 corpus connectors get their
  stroke from `<p:style><a:lnRef>` and will still draw nothing; arrowheads (`headEnd`/`tailEnd`) are
  parsed but never reach the display list. Neither affects this cluster's 9 findings, but a reviewer
  looking at `cisco-cloud-security` after the fix should not expect its connectors to appear.
- **Tests to add:** a `pptx-parse` unit test that a `p:cxnSp` in a `spTree` parses to a shape with
  geometry `line`, the right `a:xfrm`, and its `a:ln`; and a `pptx-raster` golden covering a
  zero-height and a zero-width `prst="line"` stroke, which nothing exercises today
  (`crates/pptx-raster/tests/golden/` has no line case).

## Effort

Easy - two `matches!` arms and one `or_else` in the parser, with the real work being the
write-fidelity test that pins the parser/writer index invariant.
