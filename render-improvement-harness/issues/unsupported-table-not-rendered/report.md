---
id: unsupported-table-not-rendered
title: Draw DrawingML tables instead of a "Table" placeholder
category: table
impact: high
effort: hard
confidence: high
status: open
occurrences: 19
decks: [ocp-psp-plan, project20, rollout-plan]
findings: [ocp-psp-plan/04/1, ocp-psp-plan/05/1, ocp-psp-plan/06/1, ocp-psp-plan/07/1, ocp-psp-plan/08/1, ocp-psp-plan/09/1, ocp-psp-plan/10/1, ocp-psp-plan/11/1, ocp-psp-plan/15/1, ocp-psp-plan/16/1, ocp-psp-plan/17/1, ocp-psp-plan/18/1, project20/05/1, rollout-plan/03/1, rollout-plan/04/1, rollout-plan/05/1, rollout-plan/07/1, rollout-plan/10/1, rollout-plan/12/1]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/pptx-parse/src/package.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/display_list.rs, crates/pptx-render/src/lib.rs, crates/pptx-raster/src/lib.rs, crates/pptx-edit/src/deck.rs, packages/pptx/src/render/canvas.ts, packages/pptx/src/types.ts]
---

## Symptom

Every `p:graphicFrame` carrying an `a:tbl` renders as a dashed grey rectangle with the word
"Table" centred in it -- no grid, no cell fills, no borders, no cell text. evidence-1.png
(ocp-psp-plan/04) and evidence-2.png (rollout-plan/03) show a full-slide data table replaced
by an empty box; evidence-3.png (rollout-plan/12) shows the same for a banded 11x4 plan
table. evidence-4.png zooms the top-left of ocp-psp-plan/09, where the reference's navy
header row and first four data rows sit against nothing but the placeholder's dashed edge.
Because these tables are the slide's entire content, the affected slides run 16-52% fine
pixel diff, and this is the single largest cluster in the dataset: 19 findings across 3
decks, covering all 23 tables those decks contain (273 rows, 1204 cells).

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | ocp-psp-plan/04 | 6x13 "Partner Engines" table (red header, banded rows) vs. an empty dashed box; the title is the only thing both renderers agree on. |
| 2 | rollout-plan/03 | 8x8 RACI table (navy header, grey banding, per-cell letters) vs. an empty dashed box; 44.9% fine diff. |
| 3 | rollout-plan/12 | 4x11 "Feedback Action Plan" table vs. an empty dashed box; 48.5% fine diff. |
| 4 | ocp-psp-plan/09 | 2x zoom on the header band and first rows: the reference draws fill, borders and white bold header text; the candidate draws the placeholder outline only. |

## Root cause (hypothesis)

Confirmed, and it is two gaps stacked on each other: the table's geometry and formatting are
never parsed, and the renderer has no table primitive to draw them with.

**1. Parsing keeps only the cell text.** `parse_graphic_frame` walks `a:tbl` and pushes one
`TextBody` per `a:tc` into a `Vec<Vec<TextBody>>` --
`crates/pptx-parse/src/drawing.rs:202`-`217`, with the cell loop at
`crates/pptx-parse/src/drawing.rs:204`-`215`. `a:tblGrid` / `a:gridCol@w`, `a:tr@h`,
`a:tblPr` (`firstRow`, `bandRow`, `firstCol`, `a:tableStyleId`), `a:tcPr` (`a:solidFill`,
`a:lnL`/`lnT`/`lnR`/`lnB`, `anchor`, `marL`..`marB`, `vert`) and the merge attributes
(`gridSpan`, `rowSpan`, `hMerge`, `vMerge`) are all read past and dropped. The model matches:
`GraphicFrameData::Table` has exactly one field, `rows` --
`crates/pptx-parse/src/model.rs:243`-`245`. Nothing in `crates/pptx-parse`,
`crates/pptx-render`, `crates/pptx-raster` or `crates/ooxml-drawingml` mentions `gridCol`,
`tcPr`, `tblGrid`, `tblPr`, `tableStyleId`, `gridSpan` or `tableStyles`; `grep -rn` for any
of them across those four crates returns nothing.

**2. `ppt/tableStyles.xml` is never parsed.** `parse_pptx_with_limits` builds slides,
layouts, masters, themes, charts and media (`crates/pptx-parse/src/package.rs:17`-`184`);
there is no table-style stage, and `PptxPackage` has no field for one
(`crates/pptx-parse/src/model.rs:15`-`27`). The raw bytes do survive in `parts`
(`crates/pptx-parse/src/package.rs:171`-`173`), but that field is `#[serde(skip)]`
(`crates/pptx-parse/src/model.rs:26`), so a package recovered from a collaboration update
could not read the part lazily.

**3. Layout emits a placeholder and stops.** Slide shapes always take the snapshot path
(`crates/pptx-render/src/layout.rs:230`), whose `ShapeKind::GraphicFrame` arm calls
`render_graphic_frame` (`crates/pptx-render/src/layout.rs:439`-`447`). That function plots a
chart when the graphic resolves to a chart part and otherwise pushes
`Primitive::Placeholder` (`crates/pptx-render/src/layout.rs:593`-`636`, push at `:625`),
labelled from `graphic_label`, which returns `"Table"` for the table variant
(`crates/pptx-render/src/layout.rs:1960`-`1966`). The parsed path used for layout and master
shapes does the same (`crates/pptx-render/src/layout.rs:551`-`559`), and the standalone
compile path has its own `ComposedShape::TablePlaceholder` arm
(`crates/pptx-render/src/lib.rs:213`-`215`).

**4. There is no table primitive to emit.** `Primitive` offers `Shape`, `Image`, `TextBox`,
`Placeholder` and `Chart` (placeholder variant at
`crates/pptx-render/src/display_list.rs:134`). `pptx-raster` paints a placeholder as a 1px
dashed `[5, 4]` grey rectangle plus a 12px centred label
(`crates/pptx-raster/src/lib.rs:300`-`302` dispatch,
`crates/pptx-raster/src/lib.rs:436`-`480`), and `packages/pptx/src/render/canvas.ts:80` /
`:247` do the same in the browser. So even with the grid in the model, nothing downstream
could express it.

**5. A confirmed secondary bug: one stray cell of text is drawn over the whole frame.**
After the shape-kind `match`, the snapshot path lays out `shape.text_stories.first()` across
the frame's full rectangle (`crates/pptx-render/src/layout.rs:458`). `seed_shape` seeds one
story per cell for a table frame (`crates/pptx-edit/src/deck.rs:148`-`161`), so the *first
cell only* is rendered, at the frame's origin, and every other cell is silently discarded.
Dumping the display list confirms it: ocp-psp-plan slide 4 emits
`placeholder "Table 12" (24, 64, 1225x642)` immediately followed by
`textBox (24, 64, 1225x642) #FFFFFF "Engine"`, and project20 slide 5 emits three
placeholders each shadowed by a white `"Resource Name"` textBox. It is invisible today only
because those header runs are white on a white slide; a dark first cell would paint a stray
word across the placeholder. `node_text` returns `None` for a graphic frame
(`crates/pptx-render/src/layout.rs:1772`-`1777`), so the parsed path does not have this bug.

What PowerPoint does: lay the columns out from `a:gridCol@w`, treat `a:tr@h` as a *minimum*
row height and grow the row to its tallest cell, resolve each cell's fill, borders and text
properties by layering the table style's `wholeTbl` / `band1H` / `firstRow` / `firstCol`
parts (selected by the `a:tblPr` flags) under the cell's explicit `a:tcPr`, then draw fills,
borders and per-cell text bodies clipped to their cells.

Scale and shape of the work, measured over the three decks' slide XML:

| | ocp-psp-plan | project20 | rollout-plan |
|---|---|---|---|
| tables / rows / cells | 13 / 106 / 576 | 3 / 120 / 360 | 7 / 47 / 268 |
| `firstRow` / `bandRow` | 13 / 13 | 3 / 3 | 6 / 6 |
| `a:tableStyleId` | 13 | 0 | 7 |
| cells with explicit `a:solidFill` | 131 | 360 | 0 |
| cells with explicit `lnL`..`lnB` | 346 | 360 | 0 |
| cells with `anchor` / `marL` | 105 / 324 | 360 / 360 | 93 / 72 |
| merges (`gridSpan`/`hMerge`/`rowSpan`/`vMerge`) | 16 / 16 / 21 / 35 | 0 | 0 |
| runs with an explicit colour | 399 of 1013 | 363 of 363 | 18 of 217 |

Three consequences worth planning around:

- Both extremes are real. project20 needs no style engine at all -- every cell carries its
  own fill, borders, margins and run colour -- while rollout-plan needs one for
  *everything*: zero explicit cell fills, zero explicit borders, and only 18 of 217 runs with
  a colour, so its navy header bands and white bold header text come entirely from the
  referenced style.
- Style ids do not all resolve from the package. Of the 8 distinct `a:tableStyleId` values
  used, 6 are defined in the deck's own `ppt/tableStyles.xml`; ocp-psp-plan references
  `{5940675A-B579-460E-94D1-54222C63F5DA}` (5 tables) and
  `{5202B0CA-FC54-4496-8BCA-5EF66A818D29}` (1 table), neither of which is in its
  `tableStyles.xml`. Those are PowerPoint built-in styles that live in the application, not
  the file. LibreOffice draws slide 09's navy header from its own built-in table, so matching
  the reference on those 6 tables needs a small built-in style table or a documented
  approximation.
- Every row in all three decks carries `h`, so "trust `a:tr@h`" gets close -- but
  LibreOffice grows rows to fit their content (its ocp-psp-plan/04 table overflows the slide
  bottom, visible in evidence-1.png), so matching it means measuring cell text and expanding.

## Verification

Re-render ocp-psp-plan slides 04-11 and 15-18, rollout-plan slides 03, 04, 05, 07, 10, 12,
and project20 slide 05. These slides are table-dominated, so a correct implementation should
take them from 16-52% fine diff to a small residual. project20/05 is the fairest early check
because its cells are fully self-describing; rollout-plan/03 and /12 are the check that the
table-style layer works at all. ocp-psp-plan/04, /09 and the other four tables on
unresolvable built-in style ids will keep a header-colour diff until a built-in style
fallback exists -- judge those on grid, text and banding, not header hue. project20/05 also
carries four unrelated findings (title size, run `gradFill`, a missing connector, an
`lo-suspect` footer), so its residual will not reach zero from this fix alone.

No test covers table rendering today. The nearest coverage is
`crates/pptx-raster/tests/golden.rs:327`-`343` (`golden_placeholder`, which renders exactly
this "Table" placeholder into `crates/pptx-raster/tests/golden/placeholder.png` and can stay
as the fallback case) and the chart-frame test at
`crates/pptx-render/src/layout.rs:2379`, which asserts a graphic frame without its part keeps
a placeholder. Nothing in `crates/pptx-parse` asserts anything about `a:tbl` beyond the
text-target lookup in `crates/pptx-parse/src/write.rs:1274`-`1278`, so the fix needs new
tests at every layer.
