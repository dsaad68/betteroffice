# pptx: hidden="1" shapes drawn anyway

**Describe the bug**

A shape whose `p:cNvPr` carries `hidden="1"` is drawn as if it were visible. On nine of the ten
compared `project17` slides the same authoring leftover - a full-width text box named
`3. Unit of measure` (id 8) - paints a gold caption straight across the title band
(evidence-1.png, evidence-4.png), and on four of them a 16x16 px accent square named `Rectangle 3`
(id 4) sits in the title bar's top-left corner (evidence-2.png). On `cisco-cloud-security` slide 7
the flag is on a *group* (`Group 5`, id 6) whose two large "wing" freeforms then blanket most of
the diagram in flat `#A3A3A3` (evidence-3.png). LibreOffice and PowerPoint suppress all of these.

Seen on 11 slides across 2 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project17/05** `3. Unit of measure` (id 8, `hidden="1"`) drawn in gold over the title's second line; absent in the reference

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/hidden-shape-drawn-anyway/evidence-1.png)

**2. project17/12** top-left corner magnified: `Rectangle 3` (id 4, `hidden="1"`) drawn as a lighter accent square on the title band

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/hidden-shape-drawn-anyway/evidence-2.png)

**3. cisco-cloud-security/07** the whole slide: the hidden `Group 5` (id 6) wing freeforms wash the diagram in opaque gray

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/hidden-shape-drawn-anyway/evidence-3.png)

**4. project17/08** the same hidden id 8 caption on a different slide - the failure repeats on nine of ten slides

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/hidden-shape-drawn-anyway/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 7
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 4, 5, 6, 7, 8, 9, 10, 11, 12, 13

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

`hidden` is parsed correctly and is honoured on the layout/master path, but it is dropped at the
model to CRDT-document boundary, so the slide's own shapes - which render from the snapshot, not
from the parsed model - never see it.

1. Parse is fine. `parse_base` reads the attribute for all four shape kinds
   ([`crates/pptx-parse/src/drawing.rs:288`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L288), reached from `parse_shape`
   [`crates/pptx-parse/src/drawing.rs:147`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L147), `parse_picture` `:180`, `parse_graphic_frame` `:247`,
   `parse_group` `:260`) into `ShapeBase::hidden` ([`crates/pptx-parse/src/model.rs:166`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L166)).
2. Seeding drops it. `seed_shape` ([`crates/pptx-edit/src/deck.rs:86`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L86)) copies `base.id`, `name`,
   the whole transform and the placeholder into the Yjs shape map
   ([`crates/pptx-edit/src/deck.rs:103-119`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L103-L119)) but never writes `hidden`.
3. The snapshot therefore has no such field. `ShapeSnapshot`
   ([`crates/pptx-edit/src/model.rs:99-121`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L99-L121)) carries `flip_h`/`flip_v`/`placeholder` but no
   `hidden`, and the reader `snapshot_shape` ([`crates/pptx-edit/src/deck.rs:804`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L804)) cannot read one.
4. The renderer therefore cannot check it. `render_snapshot_shape`
   ([`crates/pptx-render/src/layout.rs:340`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L340)) - the function that renders every slide shape, called
   from the `for shape in &deck_slide.shapes` loop at [`crates/pptx-render/src/layout.rs:230`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L230) - has
   no visibility guard; its group branch ([`crates/pptx-render/src/layout.rs:361-373`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L361-L373)) recurses into
   children unconditionally, which is why the hidden `Group 5` still emits both of its freeforms.

The contrast is the master/layout path: `render_parsed_shape` renders from `ShapeNode` and does
guard, [`crates/pptx-render/src/layout.rs:487`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L487):

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
([`crates/pptx-render/src/layout.rs:2547`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2547)).

Adjacent, not part of this cluster: slide-level `show="0"` (hidden *slides*) is not parsed either -
[`crates/pptx-parse/src/package.rs:65`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/package.rs#L65) reads `showMasterSp` but no `show` attribute, and `Slide`
([`crates/pptx-parse/src/model.rs:83`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L83)) has no field for it.

**Suggested fix**

Carry `ShapeBase::hidden` through the CRDT document to `ShapeSnapshot`, then guard
`render_snapshot_shape` the same way `render_parsed_shape` is already guarded. Four small edits:

1. [`crates/pptx-edit/src/deck.rs:112`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L112) - in `seed_shape`, write the flag next to `flipH`/`flipV`:
   `shape_map.insert(txn, "hidden", base.hidden);`
2. [`crates/pptx-edit/src/model.rs:110`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L110) - add `pub hidden: bool,` to `ShapeSnapshot`, next to
   `flip_v`.
3. [`crates/pptx-edit/src/deck.rs:815`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L815) - in `snapshot_shape`, read it the tolerant way the other
   booleans are read: `hidden: map_bool(&shape, txn, "hidden").unwrap_or_default(),`. The
   `unwrap_or_default()` keeps documents seeded before this change loading unchanged (they simply
   read `false`, i.e. today's behaviour), so no `SCHEMA_VERSION` bump is needed - `migrate_doc`
   ([`crates/pptx-edit/src/deck.rs:669`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L669)) only restamps `packageJson`, it does not reseed shapes.
4. [`crates/pptx-render/src/layout.rs:345`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L345) - guard at the top of `render_snapshot_shape`, before the
   group branch at `:361` so a hidden group takes its whole subtree with it.

Mirror the new field in the two consumers that spell the snapshot out by hand:
[`packages/pptx/src/types.ts:66`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L66) (`hidden: boolean;`) and, if the flag should be visible to callers,
[`bindings/python-pptx/src/lib.rs:488`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/bindings/python-pptx/src/lib.rs#L488) next to `flip_v`.

An alternative that touches only the renderer - resolving `original` (already computed at
[`crates/pptx-render/src/layout.rs:346`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L346)) and testing `node_base(original).hidden` - is *not*
recommended: it fails for shapes with `source_id == 0` (added after open), it re-reads the parsed
model that the snapshot is supposed to be the authority for, and it leaves the flag invisible to
`packages/pptx` and the Python binding.

```rust
// crates/pptx-edit/src/deck.rs, seed_shape
shape_map.insert(txn, "flipV", base.transform.flip_v);
shape_map.insert(txn, "hidden", base.hidden);

// crates/pptx-edit/src/deck.rs, snapshot_shape
flip_v: map_bool(&shape, txn, "flipV").unwrap_or_default(),
hidden: map_bool(&shape, txn, "hidden").unwrap_or_default(),

// crates/pptx-render/src/layout.rs, render_snapshot_shape
fn render_snapshot_shape(&mut self, shape: &ShapeSnapshot, space: Space) -> Result<(), RenderError> {
    self.charge_shape()?;
    if shape.hidden {
        return Ok(());
    }
    ...
```

Note the guard sits *after* `charge_shape()` so the budget still counts the shape, matching
`render_parsed_shape` ([`crates/pptx-render/src/layout.rs:485-489`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L485-L489)) exactly.

Risks and tests to add:

- Hidden shapes disappear from `hit_regions` too, so they stop being selectable in the editor. That
  matches PowerPoint (a hidden shape is only reachable through the selection pane) but is a visible
  behaviour change for `packages/pptx-react`; check the hit-test tests in
  `crates/pptx-render/src/layout.rs` and `packages/pptx-react/src/interactions.test.ts`.
- `ShapeSnapshot` is serialized (`serde`, `camelCase`), so any persisted snapshot fixture or
  golden JSON gains a `hidden` key. Grep the `tests/` trees for stored snapshots before landing.
- `add_shape` / `add_text_box` ([`crates/pptx-edit/src/deck.rs:365`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L365), `:423`) create shapes without
  the key; `unwrap_or_default()` makes them visible, which is correct.
- Round trip: the write path patches existing `cNvPr` elements in place and only synthesizes new
  ones for shapes added through the API ([`crates/pptx-parse/src/write.rs:1726`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1726)), so `hidden="1"`
  should survive untouched - worth confirming with a save-reopen of `project17` all the same.
- Tests to add: a `pptx-render` case asserting a `hidden="1"` slide shape emits no primitive and no
  hit region, one asserting a hidden *group* suppresses its children, and a `pptx-edit` case
  asserting the snapshot round-trips the flag. None exist today.

**How to verify**

Re-render `project17` slides 04-13 and `cisco-cloud-security` slide 07 with
`.venv/bin/python render-improvement-harness/scripts/render_bo.py` plus `diff.py`.

- `cisco-cloud-security/07` should lose the gray wash; the 41.13% diff should fall sharply, with
  the hot cells r4c4 (97.9%), r3c4 (84.8%) and r2c4 (60.2%) dropping to whatever the remaining
  `grpFill`/`custGeom` issues account for.
- `project17/04-13` should lose the gold `Unit of measure` band under the title and the top-left
  accent square; the diff bands around y in [0.083, 0.125] should clear.
- The quickest unit-level check: assert that a slide shape with `hidden="1"` emits no primitive,
  mirroring the existing master/layout behaviour at [`crates/pptx-render/src/layout.rs:487`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L487). There
  is no such test today, so one has to be added rather than extended.
- Also confirm the round trip: `pptx-edit`'s save path must not resurrect or drop the attribute on
  decks that were opened and written back.

**Additional context**

none.

Related issues found in the same run: none.

Files most likely involved: `crates/pptx-edit/src/model.rs`, `crates/pptx-edit/src/deck.rs`, `crates/pptx-render/src/layout.rs`, `packages/pptx/src/types.ts`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/hidden-shape-drawn-anyway/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
