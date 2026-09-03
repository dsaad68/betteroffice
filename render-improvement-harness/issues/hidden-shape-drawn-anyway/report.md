---
id: hidden-shape-drawn-anyway
title: Carry cNvPr hidden through the deck snapshot and skip hidden shapes
category: hidden
impact: high
effort: easy
confidence: high
status: open
occurrences: 11
decks: [cisco-cloud-security, project17]
findings: [cisco-cloud-security/07/1, project17/04/2, project17/05/1, project17/06/1, project17/07/1, project17/08/1, project17/09/2, project17/10/1, project17/11/1, project17/12/1, project17/13/1]
files: [crates/pptx-edit/src/model.rs, crates/pptx-edit/src/deck.rs, crates/pptx-render/src/layout.rs, packages/pptx/src/types.ts]
---

## Symptom

A shape whose `p:cNvPr` carries `hidden="1"` is drawn as if it were visible. On nine of the ten
compared `project17` slides the same authoring leftover - a full-width text box named
`3. Unit of measure` (id 8) - paints a gold caption straight across the title band
(evidence-1.png, evidence-4.png), and on four of them a 16x16 px accent square named `Rectangle 3`
(id 4) sits in the title bar's top-left corner (evidence-2.png). On `cisco-cloud-security` slide 7
the flag is on a *group* (`Group 5`, id 6) whose two large "wing" freeforms then blanket most of
the diagram in flat `#A3A3A3` (evidence-3.png). LibreOffice and PowerPoint suppress all of these.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project17/05 | `3. Unit of measure` (id 8, `hidden="1"`) drawn in gold over the title's second line; absent in the reference |
| 2 | project17/12 | top-left corner magnified: `Rectangle 3` (id 4, `hidden="1"`) drawn as a lighter accent square on the title band |
| 3 | cisco-cloud-security/07 | the whole slide: the hidden `Group 5` (id 6) wing freeforms wash the diagram in opaque gray |
| 4 | project17/08 | the same hidden id 8 caption on a different slide - the failure repeats on nine of ten slides |

## Root cause (confirmed)

`hidden` is parsed correctly and is honoured on the layout/master path, but it is dropped at the
model to CRDT-document boundary, so the slide's own shapes - which render from the snapshot, not
from the parsed model - never see it.

1. Parse is fine. `parse_base` reads the attribute for all four shape kinds
   (`crates/pptx-parse/src/drawing.rs:288`, reached from `parse_shape`
   `crates/pptx-parse/src/drawing.rs:147`, `parse_picture` `:180`, `parse_graphic_frame` `:247`,
   `parse_group` `:260`) into `ShapeBase::hidden` (`crates/pptx-parse/src/model.rs:166`).
2. Seeding drops it. `seed_shape` (`crates/pptx-edit/src/deck.rs:86`) copies `base.id`, `name`,
   the whole transform and the placeholder into the Yjs shape map
   (`crates/pptx-edit/src/deck.rs:103-119`) but never writes `hidden`.
3. The snapshot therefore has no such field. `ShapeSnapshot`
   (`crates/pptx-edit/src/model.rs:99-121`) carries `flip_h`/`flip_v`/`placeholder` but no
   `hidden`, and the reader `snapshot_shape` (`crates/pptx-edit/src/deck.rs:804`) cannot read one.
4. The renderer therefore cannot check it. `render_snapshot_shape`
   (`crates/pptx-render/src/layout.rs:340`) - the function that renders every slide shape, called
   from the `for shape in &deck_slide.shapes` loop at `crates/pptx-render/src/layout.rs:230` - has
   no visibility guard; its group branch (`crates/pptx-render/src/layout.rs:361-373`) recurses into
   children unconditionally, which is why the hidden `Group 5` still emits both of its freeforms.

The contrast is the master/layout path: `render_parsed_shape` renders from `ShapeNode` and does
guard, `crates/pptx-render/src/layout.rs:487`:

```rust
if node_base(shape).hidden {
    return Ok(());
}
```

so the same flag works on master and layout shapes and only fails on slide shapes.

Confirmed against the XML and against the running renderer at HEAD (36a9235):

- `decks/project17/xml/05/slide.xml` has `<p:cNvPr id="8" name="3. Unit of measure" hidden="1"/>`,
  and `decks/cisco-cloud-security/xml/07/slide.xml` has
  `<p:cNvPr id="6" name="Group 5" hidden="1"/>`.
- Rendering `project17` slide 5 through the Python binding yields a display list containing
  `{'kind': 'shape', 'objectId': 8, 'name': '3. Unit of measure', ...}` plus its `textBox`
  primitive; `cisco-cloud-security` slide 7 yields four primitives for object ids 264 and 265, the
  two children of the hidden group, filled `{'kind': 'solid', 'color': '#A3A3A3'}`.
- The public snapshot exposes no visibility at all: `Presentation.slide(4).shapes` lists
  `3. Unit of measure` and `Object 13` (also `hidden="1"`) as ordinary shapes.

No test anywhere in the tree exercises `hidden` on the shape path - the only two mentions in
`crates/pptx-render/src/layout.rs` are the guard itself and a `hidden: false` literal in a fixture
(`crates/pptx-render/src/layout.rs:2547`).

Adjacent, not part of this cluster: slide-level `show="0"` (hidden *slides*) is not parsed either -
`crates/pptx-parse/src/package.rs:65` reads `showMasterSp` but no `show` attribute, and `Slide`
(`crates/pptx-parse/src/model.rs:83`) has no field for it.

## Verification

Re-render `project17` slides 04-13 and `cisco-cloud-security` slide 07 with
`.venv/bin/python render-improvement-harness/scripts/render_bo.py` plus `diff.py`.

- `cisco-cloud-security/07` should lose the gray wash; the 41.13% diff should fall sharply, with
  the hot cells r4c4 (97.9%), r3c4 (84.8%) and r2c4 (60.2%) dropping to whatever the remaining
  `grpFill`/`custGeom` issues account for.
- `project17/04-13` should lose the gold `Unit of measure` band under the title and the top-left
  accent square; the diff bands around y in [0.083, 0.125] should clear.
- The quickest unit-level check: assert that a slide shape with `hidden="1"` emits no primitive,
  mirroring the existing master/layout behaviour at `crates/pptx-render/src/layout.rs:487`. There
  is no such test today, so one has to be added rather than extended.
- Also confirm the round trip: `pptx-edit`'s save path must not resurrect or drop the attribute on
  decks that were opened and written back.
