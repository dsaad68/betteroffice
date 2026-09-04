# pptx: Run-level solidFill/formatting not scoped to its own run

**Describe the bug**

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

Seen on 6 slides across 2 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project17/13** both directions in one shape: gold 2nd runs drop to black, and the gold 1st run leaks over the black run that follows it

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-solidfill-scope-bug/evidence-1.png)

**2. project17/06** the lead-in run's gold covers the whole first visual line; black only resumes where the text wraps to line 2

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-solidfill-scope-bug/evidence-2.png)

**3. project17/07** "Business Technology office" (gold, `b="1"`) paints black after a black run; "Independent" (gold) leaks onto "– with no ties to vendors"

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-solidfill-scope-bug/evidence-3.png)

**4. project17/10** the title run "Transformation" loses gold, bold and underline together

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-solidfill-scope-bug/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 5, 6, 7, 10, 13
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 3

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(4, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

Confirmed in the code, not a guess.

Per-run styling survives parsing and resolution: `RunProperties.color` is a real field
([`crates/pptx-parse/src/model.rs:344`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L344)), the story snapshot keeps one `TextStyle` per run
([`crates/pptx-edit/src/story.rs:445`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L445)), `resolve_content` resolves a `ResolvedStyle` for every run
([`crates/pptx-render/src/layout.rs:970-981`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L970-L981)), and every `ShapedCluster` carries both its
`run_index` and a clone of that style ([`crates/pptx-render/src/layout.rs:1266-1276`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1266-L1276),
[`crates/pptx-render/src/layout.rs:1419-1430`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1419-L1430)). Shaping is therefore correct — glyphs are shaped
with each run's own face and size.

The loss happens when clusters are folded back into display-list runs.
`positioned_runs` ([`crates/pptx-render/src/layout.rs:1472`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1472)) decides whether a cluster joins the
previous `PositionedTextRun` with:

```rust
// [`crates/pptx-render/src/layout.rs:1484-1486`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1484-L1486)
let append = output.last().is_some_and(|run| {
    run.end == cluster.start && run.font_id == cluster.style.face.id.to_u32()
});
```

Only text contiguity and font id are compared. `color`, `font_size_px`, `bold`, `italic` and
`underline` are copied from the *first* cluster of the merged run
([`crates/pptx-render/src/layout.rs:1487-1502`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1487-L1502)) and are never re-checked. Adjacent runs in a
paragraph are always contiguous, because `resolve_content` assigns consecutive story offsets
([`crates/pptx-render/src/layout.rs:974-975`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L974-L975)), so font id is the only thing standing between two
differently-coloured runs and a merge — and a colour change alone never changes the font id.

That covers project17/06, /07, /13 and project20/03 directly: in slide 06 the three runs are all
`sz="1400"`, `+mj-lt`, non-bold and differ only in `srgbClr` (`9A743A`, `002960`, `000000`), so all
three fold into one gold run.

`font_id` does not even separate bold from regular reliably. `resolve_face` falls back to the
regular face and then to the first registered face when the requested `(family, bold, italic)`
triple is missing ([`crates/pptx-render/src/layout.rs:245-257`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L245-L257)), so a bold run in an unregistered
family resolves to the same `FontId` as its regular neighbour and merges. That is why
project17/10's `b="1" u="sng"` title run merges into the plain run before it — the same slide's
title is already rendering in the fallback face (see project17/05/2, title metrics), which makes
the bold and regular ids identical.

Both consumers paint one `PositionedTextRun` with a single colour, size and underline flag, so the
merge is what reaches pixels: [`crates/pptx-raster/src/font.rs:85`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/font.rs#L85) (fill), `:100` (glyph size),
`:109` (underline), and [`packages/pptx/src/render/canvas.ts:235-239`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L235-L239) for the browser backend.

Not confirmed: whether `project17/05/4`'s "colour dropped entirely in TextBox 27" also involves a
second cause. The bleed/drop pattern in the other four boxes on that slide is this bug; TextBox 27
rendering fully black was not traced run-by-run, and slide 05 additionally suffers from the
bullet-glyph and title-font issues tracked in other clusters.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Give the merge in `positioned_runs` ([`crates/pptx-render/src/layout.rs:1472`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1472)) the identity it
actually needs. A `PositionedTextRun` is a paint unit, so a cluster may only join the previous one
when it comes from the *same source run* — `ShapedCluster` already carries `run_index`
([`crates/pptx-render/src/layout.rs:1271`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1271)), and `positioned_runs` is only ever called with a slice
of clusters from one paragraph ([`crates/pptx-render/src/layout.rs:1246`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1246)), so `run_index` uniquely
identifies the run within that slice. Adding `cluster.run_index` to the append predicate alongside
the existing contiguity check is the whole fix; the per-cluster `ResolvedStyle` is already correct,
nothing new has to be plumbed.

If keeping the run count down matters (identical adjacent runs currently collapse into one), merge
on style equality instead: derive `PartialEq` on `ResolvedStyle` and compare `cluster.style` with
the style that opened the current output run, kept in a local alongside `output`. That keeps
same-styled neighbours merged while still splitting on any colour, size, weight or underline
change. Either form fixes every finding in the cluster.

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

Risks and tests to add:

- More `PositionedTextRun`s per line on decks that split text into many same-styled runs. Nothing
  is quadratic here and the raster/canvas backends iterate runs linearly
  ([`crates/pptx-raster/src/font.rs:70`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/font.rs#L70), [`packages/pptx/src/render/canvas.ts:235`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L235)), but display-list
  snapshots or JSON-size assertions in `crates/pptx-render`, `crates/pptx-wasm` and
  `packages/pptx/src/render/canvas.test.ts` may need updating. The style-equality variant avoids
  this.
- Underline geometry is drawn per run over `run.x .. run.x + run.width`
  ([`crates/pptx-raster/src/font.rs:115-134`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/font.rs#L115-L134)). Splitting runs splits the underline into abutting
  rects; they are adjacent and same-coloured, so this should be invisible, but an underlined run
  spanning a split is worth eyeballing.
- Tests to add in the [`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2008) module: a paragraph with two runs of
  the same registered family differing only in `solidFill` must produce two positioned runs with
  the two colours; a bold run in a family with no bold face registered must keep `bold: true` and
  its own colour rather than inheriting the neighbour's. A raster golden covering the
  project17/13-style "black, gold, black" line would guard the visual result.

**How to verify**

Re-render project17 slides 06, 07, 10 and 13 and project20 slide 03
(`.venv/bin/python render-improvement-harness/scripts/pipeline.py`). Slide 13 is the cleanest
signal: every gold emphasis span in the reference must be gold in the candidate, and
"and templates" must be black; its `fine_pct` should fall from 6.4%. Slide 06's first bullet line
must be gold only up to "Industry expertise". Slide 10's title must show "Transformation" gold,
bold and underlined; note that the residual title-width diff on slides 05/10 belongs to the
font-fallback issue and will not disappear.

There is no existing test covering run identity in `positioned_runs`
([`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2008) test module has none). The layout test fixture registers
only `Arial` regular and bold ([`crates/pptx-render/src/layout.rs:2017-2023`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2017-L2023)), which is enough to
build a two-run same-face different-colour paragraph and assert two `PositionedTextRun`s with the
two distinct colours.

**Additional context**

none.

Related issues found in the same run: none.

Files most likely involved: `crates/pptx-render/src/layout.rs`, `crates/pptx-raster/src/font.rs`, `packages/pptx/src/render/canvas.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-solidfill-scope-bug/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
