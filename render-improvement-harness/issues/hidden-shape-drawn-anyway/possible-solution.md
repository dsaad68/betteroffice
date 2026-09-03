# Possible solution: hidden-shape-drawn-anyway

## Approach

Carry `ShapeBase::hidden` through the CRDT document to `ShapeSnapshot`, then guard
`render_snapshot_shape` the same way `render_parsed_shape` is already guarded. Four small edits:

1. `crates/pptx-edit/src/deck.rs:112` - in `seed_shape`, write the flag next to `flipH`/`flipV`:
   `shape_map.insert(txn, "hidden", base.hidden);`
2. `crates/pptx-edit/src/model.rs:110` - add `pub hidden: bool,` to `ShapeSnapshot`, next to
   `flip_v`.
3. `crates/pptx-edit/src/deck.rs:815` - in `snapshot_shape`, read it the tolerant way the other
   booleans are read: `hidden: map_bool(&shape, txn, "hidden").unwrap_or_default(),`. The
   `unwrap_or_default()` keeps documents seeded before this change loading unchanged (they simply
   read `false`, i.e. today's behaviour), so no `SCHEMA_VERSION` bump is needed - `migrate_doc`
   (`crates/pptx-edit/src/deck.rs:669`) only restamps `packageJson`, it does not reseed shapes.
4. `crates/pptx-render/src/layout.rs:345` - guard at the top of `render_snapshot_shape`, before the
   group branch at `:361` so a hidden group takes its whole subtree with it.

Mirror the new field in the two consumers that spell the snapshot out by hand:
`packages/pptx/src/types.ts:66` (`hidden: boolean;`) and, if the flag should be visible to callers,
`bindings/python-pptx/src/lib.rs:488` next to `flip_v`.

An alternative that touches only the renderer - resolving `original` (already computed at
`crates/pptx-render/src/layout.rs:346`) and testing `node_base(original).hidden` - is *not*
recommended: it fails for shapes with `source_id == 0` (added after open), it re-reads the parsed
model that the snapshot is supposed to be the authority for, and it leaves the flag invisible to
`packages/pptx` and the Python binding.

## Sketch

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
`render_parsed_shape` (`crates/pptx-render/src/layout.rs:485-489`) exactly.

## Risks

- Hidden shapes disappear from `hit_regions` too, so they stop being selectable in the editor. That
  matches PowerPoint (a hidden shape is only reachable through the selection pane) but is a visible
  behaviour change for `packages/pptx-react`; check the hit-test tests in
  `crates/pptx-render/src/layout.rs` and `packages/pptx-react/src/interactions.test.ts`.
- `ShapeSnapshot` is serialized (`serde`, `camelCase`), so any persisted snapshot fixture or
  golden JSON gains a `hidden` key. Grep the `tests/` trees for stored snapshots before landing.
- `add_shape` / `add_text_box` (`crates/pptx-edit/src/deck.rs:365`, `:423`) create shapes without
  the key; `unwrap_or_default()` makes them visible, which is correct.
- Round trip: the write path patches existing `cNvPr` elements in place and only synthesizes new
  ones for shapes added through the API (`crates/pptx-parse/src/write.rs:1726`), so `hidden="1"`
  should survive untouched - worth confirming with a save-reopen of `project17` all the same.
- Tests to add: a `pptx-render` case asserting a `hidden="1"` slide shape emits no primitive and no
  hit region, one asserting a hidden *group* suppresses its children, and a `pptx-edit` case
  asserting the snapshot round-trips the flag. None exist today.

## Effort

easy - four one-line changes across three crates plus two mirrored type declarations, no schema
bump and no new parsing; most of the work is the three new tests.
