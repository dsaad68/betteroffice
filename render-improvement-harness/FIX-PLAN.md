# Fix plan

The order to land the 35 issues in, and which of them are not separate pull requests at all.

`issues/INDEX.md` ranks by impact and effort, which answers "what is worth doing". It does not
answer "what can be done next", because the fixes are heavily entangled: 27 of the 35 issue
reports reference another issue, and three files carry most of the work.

| issues touching it | file |
|---|---|
| 29 | `crates/pptx-render/src/layout.rs` |
| 22 | `crates/pptx-parse/src/drawing.rs` |
| 17 | `crates/pptx-parse/src/model.rs` |

This document is the dependency pass over those reports. Every ordering claim below is quoted
from, or traceable to, the investigator's own report; none of it is inferred from the ranking.

## Method

Each issue report and solution document was scanned for references to other issues, and each
reference classified by the sentence that carries it:

- **blocks** — the dependent fix produces no visible change, or a wrong one, until the other lands.
- **same change** — both need the same new field or predicate; splitting them means writing it twice.
- **duplicate** — the finding belongs to another cluster; there is no separate fix.
- **shares a file** — independent behaviour, contending edits; sequence rather than parallelise.
- **verification only** — the fix is independent but its slides cannot be judged until the other lands.

## Not separate pull requests

| issue | disposition |
|---|---|
| `transform-group-child-rotation-scale-wrong` | Already marked `status: duplicate`. The group child transform is correct; the shape is a `custGeom` drawn as its bounding box. Fold the finding into `geometry-custom-collapses-to-bbox`. |
| `text-inheritance-layout-lststyle-ignored` + `text-bullets-char-indent-dropped` | One pull request. Both need `a:lstStyle` parsed into `TextBody`; the report says plainly, "needs this same field, so land it once." |
| `chart-axis-position-swapped` + `chart-category-order-reversed` | One pull request. Both introduce the same `PlotFamily::transposed()` predicate, and each alone leaves the chart wrong in the other axis. |
| `text-run-props-misc-property-ignored` | Half of it is a duplicate: the italic finding is the same font-fallback defect as `text-run-props-bold-ignored` and needs no extra work. Only the superscript half survives as a change. |

## Constraints that set the order

**The text metrics chain must be strictly sequential.** Bold, character spacing, line spacing,
autofit and font substitution each change glyph advances or line boxes on every deck, so each one
invalidates the raster goldens and the harness baseline. Two of them in flight will conflict on
the golden files and each will invalidate the other's measurements.

**Two fixes make slides look worse before they look better.** Resolving `grpFill` gives the
`custGeom` icons a fill, and until their path is parsed each becomes a solid coloured rectangle.
The report is explicit: "Judge those slides only after `geometry-custom-collapses-to-bbox` is
fixed, or gate the check on the bar slides." Treat a rising diff there as expected, not as a
regression.

**One fix must not land alone.** `text-font-substitution-issues` corrects a theme lookup so
`+mj-lt` stops resolving to the minor font. Only one deck has an inverted theme; the other eight
are currently masked by the same bug and would regress. Its family-substitution step, which maps
`Calibri Light` to `Calibri`, is what prevents that and must ship in the same change.

**Chart work is single-file and cannot be parallelised.** Five chart issues edit
`crates/ooxml-drawingml/src/chart/geometry.rs`, and the legend and axis-position fixes touch the
same six lines of plot-rectangle arithmetic. The legend report says to sequence them.

## The plan

Tracks are independent of each other and can run in parallel worktrees. Within a track, order is
mandatory.

**Base every branch on `feat/pptx-screenshot`, not on `main`.** The rasterizer the harness measures
with is that branch, proposed upstream as pull request 264 and still open, and `crates/pptx-raster`
does not exist on `main` at all. A branch cut from `main` cannot render a slide, so nothing could be
verified on it. That branch carries the rasterizer and none of the harness, which is exactly the
base a fix wants. Switch to `main` once 264 merges. Never base on the harness branch itself: it
carries hundreds of report files that must not reach a pull request.

### Track A — parse gaps with no metric side effects

Safe to start immediately, in any order among themselves.

| # | issue | note |
|---|---|---|
| A1 | `hidden-shape-drawn-anyway` | Self-contained. Carries the flag through the edit snapshot and guards the renderer. |
| A2 | `fill-grpfill-not-resolved` | Verify on the bar slides only; the icon slides regress until B1. |
| A3 | `line-zero-extent-skipped` | Parser and writer must change together, or saving desynchronises and corrupts decks. |
| A4 | `text-slidenum-field-not-evaluated` | Layout only. |
| A5 | `fill-alpha-modifier-ignored` | Unblocks the shadow colour in C3. |
| A6 | `geometry-preset-adj-values-wrong` | Isolated in one file, no dependants. |

### Track B — blocked on Track A

| # | issue | blocked on | why |
|---|---|---|---|
| B1 | `geometry-custom-collapses-to-bbox` | A2 | "Blocked behind `fill-grpfill-not-resolved`." Absorbs the duplicate transform issue. |
| B2 | `line-stroke-color-resolution-broken` | A3 | "Only becomes visible once `line-zero-extent-skipped` has landed"; until connectors parse there is nothing to stroke. |
| B3 | `unsupported-custgeom-picturefill-wordmark-not-drawn` | B1 | Needs the path parsed and a picture fill on a shape. Landing either alone leaves the deck blank or paints the photo across a full rectangle. |

### Track C — pictures and effects

| # | issue | blocked on |
|---|---|---|
| C1 | `picture-srcrect-crop-ignored` | none |
| C2 | `picture-blip-duotone-bilevel-not-applied` | C1, whose crop shares the same evidence |
| C3 | `effects-prsttxwarp-and-outershdw-ignored` | A5; the shadow colour needs alpha or it paints as a solid blob |
| C4 | `picture-fill-fails-to-render` | none, but it is a new metafile decoder and the largest single piece of work here |

### Track D — text, strictly sequential

| # | issue | note |
|---|---|---|
| D1 | `text-run-props-bold-ignored` | Filed upstream as issue 266. Also fixes the italic half of the miscellaneous run-properties issue. |
| D2 | `text-font-substitution-issues` | After D1, and must include family substitution. |
| D3 | `text-run-props-solidfill-scope-bug` | One predicate; run coalescing currently merges neighbouring runs that share a font. |
| D4 | `text-inheritance-layout-lststyle-ignored` + `text-bullets-char-indent-dropped` | One pull request; shared `a:lstStyle` field. |
| D5 | `text-bullets-autonum-not-drawn` | "Lands on top of" D4's marker plumbing. |
| D6 | `text-layout-master-lnspc-ignored` | Moves text in every deck. |
| D7 | `text-overflow-autofit-not-handled` | Three defects in one; re-baselines goldens and the canvas tests. |
| D8 | `text-run-props-spc-ignored` | Crosses five crates; the writer deletes modelled attributes left unset, so a half-threaded field would strip tracking from saved decks. |
| D9 | `text-run-props-misc-property-ignored` | Superscript only by this point. |

### Track E — colour resolution

| # | issue | note |
|---|---|---|
| E1 | `theme-color-scheme-color-resolution-broken` | Two defects sharing one symptom. Split it: parsing `p:style/a:fontRef` clears 17 of the 20 findings and is the easy half; the colour map override is the harder half. Consider landing the whole shape-style matrix, meaning `fontRef`, `fillRef` and `lnRef`, as one change, since B2 needs `lnRef` too. |
| E2 | `text-run-props-gradfill-not-resolved` | Run properties read only `solidFill`, so gradient-filled text falls back to the theme colour. Colour only, so it stays out of the metrics chain in Track D. Two cautions: the writer replaces any fill with a `solidFill` whenever a colour is set, so a naive fix rewrites authored gradients as solid on save and needs a marker field to avoid it; and D4 depends on this for two of its findings, whose sizes it fixes but whose colours it cannot. |

### Track F — charts, strictly sequential, one file

| # | issue | note |
|---|---|---|
| F1 | `chart-dlbls-shown-when-disabled` | Independent of the rest; a synthesised default overrides an explicit off switch. |
| F2 | `chart-axis-position-swapped` + `chart-category-order-reversed` | One pull request; shared predicate. |
| F3 | `chart-legend-and-title-position-wrong` | Same plot-rectangle arithmetic as F2. |
| F4 | `chart-axis-autoscale-not-rounded` | Becomes more visible after F2, not less: the ticks move onto one horizontal row and collide. |
| F5 | `chart-minimal-chart-series-axis-broken` | Needs the chart-space fill described as section C of `fill-nonsolid-fill-types-not-resolved`. |

### Track G — larger, isolated

| # | issue | note |
|---|---|---|
| G1 | `fill-nonsolid-fill-types-not-resolved` | Three separable parts. Sorting gradient stops is one line and fixes four findings; pattern fills and the chart-space fill are separate. One of its findings belongs to A5 instead. |
| G2 | `transform-text-orientation-wrong-under-rotation` | Text must follow rotation but never mirror. An existing hit-test asserts today's wrong behaviour and will keep passing unless rewritten. |
| G3 | `unsupported-table-not-rendered` | Four layers plus a new display-list primitive and a contract bump. Ship it in the layers the report describes, validating on the deck that needs no table-style engine. |

## Verifying a fix

Do not use "the diff went down" as the pass criterion. It is wrong for at least A2, A5 and F4,
each of which is expected to leave or raise a diff until a later fix lands.

For each pull request:

1. Re-render only the decks the issue's `decks` field names, with the LibreOffice renders reused:
   `.venv/bin/python render-improvement-harness/scripts/pipeline.py <deck-id> --skip-lo`
2. Compare against the committed `diff-summary.json` on the harness branch.
3. Read the issue's own **Verification** section, which states what should change and what should
   remain. Several name the exact hot cells expected to clear.
4. Check which other issues have findings on the same slides before reading a residual as failure.

Slides whose diff drops become regression fixtures. Most of these areas have no test at all today;
the reports say so individually, so a fix usually adds tests rather than updating them.
