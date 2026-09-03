---
id: text-run-props-solidfill-scope-bug
title: Scope a positioned run to its source run instead of merging by font id
category: text-run-props
impact: medium
effort: easy
confidence: high
status: open
occurrences: 6
decks: [project17, project20]
findings: [project17/05/4, project17/06/4, project17/07/4, project17/10/2, project17/13/2, project20/03/4]
files: [crates/pptx-render/src/layout.rs, crates/pptx-raster/src/font.rs, packages/pptx/src/render/canvas.ts]
---

## Symptom

On a paragraph with several runs, the paint attributes of the *first* run on a visual line are
applied to every following run on that line: the neighbours' explicit `solidFill` (and their bold,
underline and font size) never reach the canvas. Which way it looks wrong depends on run order —
when the coloured run is second it loses its colour (evidence-1.png, "More than **120 consultants**
with significant **PMO experience**" renders solid black), and when it is first its colour bleeds
right (evidence-1.png, "Proprietary program management toolkit **and templates**" renders fully
gold). Because the runs are rebuilt per line, the correct colour reappears the moment a run's text
wraps onto a new line (evidence-2.png: line 1 is entirely gold, line 2 is black). The same merge
also swallows bold and underline (evidence-4.png, "Transformation") and font size
(project20/03/4).

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project17/13 | both directions in one shape: gold 2nd runs drop to black, and the gold 1st run leaks over the black run that follows it |
| 2 | project17/06 | the lead-in run's gold covers the whole first visual line; black only resumes where the text wraps to line 2 |
| 3 | project17/07 | "Business Technology office" (gold, `b="1"`) paints black after a black run; "Independent" (gold) leaks onto "– with no ties to vendors" |
| 4 | project17/10 | the title run "Transformation" loses gold, bold and underline together |

## Root cause (hypothesis)

Confirmed in the code, not a guess.

Per-run styling survives parsing and resolution: `RunProperties.color` is a real field
(`crates/pptx-parse/src/model.rs:344`), the story snapshot keeps one `TextStyle` per run
(`crates/pptx-edit/src/story.rs:445`), `resolve_content` resolves a `ResolvedStyle` for every run
(`crates/pptx-render/src/layout.rs:970-981`), and every `ShapedCluster` carries both its
`run_index` and a clone of that style (`crates/pptx-render/src/layout.rs:1266-1276`,
`crates/pptx-render/src/layout.rs:1419-1430`). Shaping is therefore correct — glyphs are shaped
with each run's own face and size.

The loss happens when clusters are folded back into display-list runs.
`positioned_runs` (`crates/pptx-render/src/layout.rs:1472`) decides whether a cluster joins the
previous `PositionedTextRun` with:

```rust
// crates/pptx-render/src/layout.rs:1484-1486
let append = output.last().is_some_and(|run| {
    run.end == cluster.start && run.font_id == cluster.style.face.id.to_u32()
});
```

Only text contiguity and font id are compared. `color`, `font_size_px`, `bold`, `italic` and
`underline` are copied from the *first* cluster of the merged run
(`crates/pptx-render/src/layout.rs:1487-1502`) and are never re-checked. Adjacent runs in a
paragraph are always contiguous, because `resolve_content` assigns consecutive story offsets
(`crates/pptx-render/src/layout.rs:974-975`), so font id is the only thing standing between two
differently-coloured runs and a merge — and a colour change alone never changes the font id.

That covers project17/06, /07, /13 and project20/03 directly: in slide 06 the three runs are all
`sz="1400"`, `+mj-lt`, non-bold and differ only in `srgbClr` (`9A743A`, `002960`, `000000`), so all
three fold into one gold run.

`font_id` does not even separate bold from regular reliably. `resolve_face` falls back to the
regular face and then to the first registered face when the requested `(family, bold, italic)`
triple is missing (`crates/pptx-render/src/layout.rs:245-257`), so a bold run in an unregistered
family resolves to the same `FontId` as its regular neighbour and merges. That is why
project17/10's `b="1" u="sng"` title run merges into the plain run before it — the same slide's
title is already rendering in the fallback face (see project17/05/2, title metrics), which makes
the bold and regular ids identical.

Both consumers paint one `PositionedTextRun` with a single colour, size and underline flag, so the
merge is what reaches pixels: `crates/pptx-raster/src/font.rs:85` (fill), `:100` (glyph size),
`:109` (underline), and `packages/pptx/src/render/canvas.ts:235-239` for the browser backend.

Not confirmed: whether `project17/05/4`'s "colour dropped entirely in TextBox 27" also involves a
second cause. The bleed/drop pattern in the other four boxes on that slide is this bug; TextBox 27
rendering fully black was not traced run-by-run, and slide 05 additionally suffers from the
bullet-glyph and title-font issues tracked in other clusters.

## Verification

Re-render project17 slides 06, 07, 10 and 13 and project20 slide 03
(`.venv/bin/python render-improvement-harness/scripts/pipeline.py`). Slide 13 is the cleanest
signal: every gold emphasis span in the reference must be gold in the candidate, and
"and templates" must be black; its `fine_pct` should fall from 6.4%. Slide 06's first bullet line
must be gold only up to "Industry expertise". Slide 10's title must show "Transformation" gold,
bold and underlined; note that the residual title-width diff on slides 05/10 belongs to the
font-fallback issue and will not disappear.

There is no existing test covering run identity in `positioned_runs`
(`crates/pptx-render/src/layout.rs:2008` test module has none). The layout test fixture registers
only `Arial` regular and bold (`crates/pptx-render/src/layout.rs:2017-2023`), which is enough to
build a two-run same-face different-colour paragraph and assert two `PositionedTextRun`s with the
two distinct colours.
