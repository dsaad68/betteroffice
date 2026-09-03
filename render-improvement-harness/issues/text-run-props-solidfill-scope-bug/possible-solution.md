# Possible solution: text-run-props-solidfill-scope-bug

## Approach

Give the merge in `positioned_runs` (`crates/pptx-render/src/layout.rs:1472`) the identity it
actually needs. A `PositionedTextRun` is a paint unit, so a cluster may only join the previous one
when it comes from the *same source run* — `ShapedCluster` already carries `run_index`
(`crates/pptx-render/src/layout.rs:1271`), and `positioned_runs` is only ever called with a slice
of clusters from one paragraph (`crates/pptx-render/src/layout.rs:1246`), so `run_index` uniquely
identifies the run within that slice. Adding `cluster.run_index` to the append predicate alongside
the existing contiguity check is the whole fix; the per-cluster `ResolvedStyle` is already correct,
nothing new has to be plumbed.

If keeping the run count down matters (identical adjacent runs currently collapse into one), merge
on style equality instead: derive `PartialEq` on `ResolvedStyle` and compare `cluster.style` with
the style that opened the current output run, kept in a local alongside `output`. That keeps
same-styled neighbours merged while still splitting on any colour, size, weight or underline
change. Either form fixes every finding in the cluster.

## Sketch

```rust
// crates/pptx-render/src/layout.rs, in positioned_runs
let mut output: Vec<PositionedTextRun> = Vec::new();
let mut open_run: Option<usize> = None; // source run index of output.last()
let mut cursor_x = line_x;
for cluster in clusters {
    if cluster.text == "\n" {
        continue;
    }
    let append = output
        .last()
        .is_some_and(|run| run.end == cluster.start && open_run == Some(cluster.run_index));
    if !append {
        open_run = Some(cluster.run_index);
        output.push(PositionedTextRun { /* unchanged: fields from cluster.style */ });
    }
    // ... unchanged
}
```

`font_id` drops out of the predicate: two clusters from the same run always share a face, and two
clusters from different runs must not merge even when they happen to share one.

## Risks

- More `PositionedTextRun`s per line on decks that split text into many same-styled runs. Nothing
  is quadratic here and the raster/canvas backends iterate runs linearly
  (`crates/pptx-raster/src/font.rs:70`, `packages/pptx/src/render/canvas.ts:235`), but display-list
  snapshots or JSON-size assertions in `crates/pptx-render`, `crates/pptx-wasm` and
  `packages/pptx/src/render/canvas.test.ts` may need updating. The style-equality variant avoids
  this.
- Underline geometry is drawn per run over `run.x .. run.x + run.width`
  (`crates/pptx-raster/src/font.rs:115-134`). Splitting runs splits the underline into abutting
  rects; they are adjacent and same-coloured, so this should be invisible, but an underlined run
  spanning a split is worth eyeballing.
- Tests to add in the `crates/pptx-render/src/layout.rs:2008` module: a paragraph with two runs of
  the same registered family differing only in `solidFill` must produce two positioned runs with
  the two colours; a bold run in a family with no bold face registered must keep `bold: true` and
  its own colour rather than inheriting the neighbour's. A raster golden covering the
  project17/13-style "black, gold, black" line would guard the visual result.

## Effort

easy — a one-predicate change in a single function plus two unit tests; no new data has to be
carried through layout, and the per-run styles it needs are already on the cluster.
