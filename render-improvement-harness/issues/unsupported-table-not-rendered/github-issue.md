# pptx: DrawingML tables (a:tbl) not rendered at all

**Describe the bug**

Every `p:graphicFrame` carrying an `a:tbl` renders as a dashed grey rectangle with the word
"Table" centred in it -- no grid, no cell fills, no borders, no cell text. evidence-1.png
(ocp-psp-plan/04) and evidence-2.png (rollout-plan/03) show a full-slide data table replaced
by an empty box; evidence-3.png (rollout-plan/12) shows the same for a banded 11x4 plan
table. evidence-4.png zooms the top-left of ocp-psp-plan/09, where the reference's navy
header row and first four data rows sit against nothing but the placeholder's dashed edge.
Because these tables are the slide's entire content, the affected slides run 16-52% fine
pixel diff, and this is the single largest cluster in the dataset: 19 findings across 3
decks, covering all 23 tables those decks contain (273 rows, 1204 cells).

Seen on 19 slides across 3 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort hard, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. ocp-psp-plan/04** 6x13 "Partner Engines" table (red header, banded rows) vs. an empty dashed box; the title is the only thing both renderers agree on.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-table-not-rendered/evidence-1.png)

**2. rollout-plan/03** 8x8 RACI table (navy header, grey banding, per-cell letters) vs. an empty dashed box; 44.9% fine diff.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-table-not-rendered/evidence-2.png)

**3. rollout-plan/12** 4x11 "Feedback Action Plan" table vs. an empty dashed box; 48.5% fine diff.

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-table-not-rendered/evidence-3.png)

**4. ocp-psp-plan/09** 2x zoom on the header band and first rows: the reference draws fill, borders and white bold header text; the candidate draws the placeholder outline only.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-table-not-rendered/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 18
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 5
- `rollout-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 3, 4, 5, 7, 10, 12

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(3, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

Confirmed, and it is two gaps stacked on each other: the table's geometry and formatting are
never parsed, and the renderer has no table primitive to draw them with.

**1. Parsing keeps only the cell text.** `parse_graphic_frame` walks `a:tbl` and pushes one
`TextBody` per `a:tc` into a `Vec<Vec<TextBody>>` --
[`crates/pptx-parse/src/drawing.rs:202`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L202)-`217`, with the cell loop at
[`crates/pptx-parse/src/drawing.rs:204`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L204)-`215`. `a:tblGrid` / `a:gridCol@w`, `a:tr@h`,
`a:tblPr` (`firstRow`, `bandRow`, `firstCol`, `a:tableStyleId`), `a:tcPr` (`a:solidFill`,
`a:lnL`/`lnT`/`lnR`/`lnB`, `anchor`, `marL`..`marB`, `vert`) and the merge attributes
(`gridSpan`, `rowSpan`, `hMerge`, `vMerge`) are all read past and dropped. The model matches:
`GraphicFrameData::Table` has exactly one field, `rows` --
[`crates/pptx-parse/src/model.rs:243`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L243)-`245`. Nothing in `crates/pptx-parse`,
`crates/pptx-render`, `crates/pptx-raster` or `crates/ooxml-drawingml` mentions `gridCol`,
`tcPr`, `tblGrid`, `tblPr`, `tableStyleId`, `gridSpan` or `tableStyles`; `grep -rn` for any
of them across those four crates returns nothing.

**2. `ppt/tableStyles.xml` is never parsed.** `parse_pptx_with_limits` builds slides,
layouts, masters, themes, charts and media ([`crates/pptx-parse/src/package.rs:17`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/package.rs#L17)-`184`);
there is no table-style stage, and `PptxPackage` has no field for one
([`crates/pptx-parse/src/model.rs:15`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L15)-`27`). The raw bytes do survive in `parts`
([`crates/pptx-parse/src/package.rs:171`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/package.rs#L171)-`173`), but that field is `#[serde(skip)]`
([`crates/pptx-parse/src/model.rs:26`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L26)), so a package recovered from a collaboration update
could not read the part lazily.

**3. Layout emits a placeholder and stops.** Slide shapes always take the snapshot path
([`crates/pptx-render/src/layout.rs:230`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L230)), whose `ShapeKind::GraphicFrame` arm calls
`render_graphic_frame` ([`crates/pptx-render/src/layout.rs:439`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L439)-`447`). That function plots a
chart when the graphic resolves to a chart part and otherwise pushes
`Primitive::Placeholder` ([`crates/pptx-render/src/layout.rs:593`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L593)-`636`, push at `:625`),
labelled from `graphic_label`, which returns `"Table"` for the table variant
([`crates/pptx-render/src/layout.rs:1960`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1960)-`1966`). The parsed path used for layout and master
shapes does the same ([`crates/pptx-render/src/layout.rs:551`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L551)-`559`), and the standalone
compile path has its own `ComposedShape::TablePlaceholder` arm
([`crates/pptx-render/src/lib.rs:213`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L213)-`215`).

**4. There is no table primitive to emit.** `Primitive` offers `Shape`, `Image`, `TextBox`,
`Placeholder` and `Chart` (placeholder variant at
[`crates/pptx-render/src/display_list.rs:134`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L134)). `pptx-raster` paints a placeholder as a 1px
dashed `[5, 4]` grey rectangle plus a 12px centred label
([`crates/pptx-raster/src/lib.rs:300`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L300)-`302` dispatch,
[`crates/pptx-raster/src/lib.rs:436`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L436)-`480`), and [`packages/pptx/src/render/canvas.ts:80`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L80) /
`:247` do the same in the browser. So even with the grid in the model, nothing downstream
could express it.

**5. A confirmed secondary bug: one stray cell of text is drawn over the whole frame.**
After the shape-kind `match`, the snapshot path lays out `shape.text_stories.first()` across
the frame's full rectangle ([`crates/pptx-render/src/layout.rs:458`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L458)). `seed_shape` seeds one
story per cell for a table frame ([`crates/pptx-edit/src/deck.rs:148`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L148)-`161`), so the *first
cell only* is rendered, at the frame's origin, and every other cell is silently discarded.
Dumping the display list confirms it: ocp-psp-plan slide 4 emits
`placeholder "Table 12" (24, 64, 1225x642)` immediately followed by
`textBox (24, 64, 1225x642) #FFFFFF "Engine"`, and project20 slide 5 emits three
placeholders each shadowed by a white `"Resource Name"` textBox. It is invisible today only
because those header runs are white on a white slide; a dark first cell would paint a stray
word across the placeholder. `node_text` returns `None` for a graphic frame
([`crates/pptx-render/src/layout.rs:1772`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1772)-`1777`), so the parsed path does not have this bug.

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

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Four layers, in dependency order. Each is separately shippable and each visibly improves the
harness decks, so this should land as a stack rather than one change.

**1. Parse the table (`crates/pptx-parse`).** Widen `GraphicFrameData::Table`
([`crates/pptx-parse/src/model.rs:243`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L243)-`245`) from `rows: Vec<Vec<TextBody>>` to a real
`Table` struct: `grid: Vec<i64>` from `a:tblGrid/a:gridCol@w`, `properties` from `a:tblPr`
(`first_row`, `first_col`, `last_row`, `last_col`, `band_row`, `band_col`, `style_id`), and
`rows: Vec<TableRow>` where a row carries `height: i64` (`a:tr@h`) and
`cells: Vec<TableCell>`. A `TableCell` carries the existing `TextBody`, the merge attributes
(`grid_span`, `row_span`, `h_merge`, `v_merge`), the `a:tcPr` fill (`parse_fill`,
[`crates/pptx-parse/src/drawing.rs:565`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L565)) and four optional borders. Borders need a small
variant of `parse_outline` ([`crates/pptx-parse/src/drawing.rs:624`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L624)) that takes the
`a:lnL`/`lnT`/`lnR`/`lnB` element itself rather than looking up a child named `ln`.

Fold `a:tcPr`'s `anchor`, `vert` and `marL`/`marT`/`marR`/`marB` straight into the cell's
`TextBody` (`anchor`, `vertical`, `inset_*` -- [`crates/pptx-parse/src/model.rs:266`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L266)-`278`) at
parse time. That is exactly the shape `BodyCascade` already reads
([`crates/pptx-render/src/layout.rs:769`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L769)-`780`), so cell text needs no new cascade plumbing.

Keep `rows` row-major and the cell order stable: `seed_shape` derives story ids positionally
as `story:{shape}:table:{row}:{cell}` ([`crates/pptx-edit/src/deck.rs:152`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L152)-`158`), and layout
will zip stories against cells by the same index.

**2. Parse table styles (`crates/pptx-parse/src/package.rs`).** Add a `table_styles` stage
next to the theme stage in `parse_pptx_with_limits` ([`crates/pptx-parse/src/package.rs:17`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/package.rs#L17)),
reading `ppt/tableStyles.xml` into `PptxPackage.table_styles`
([`crates/pptx-parse/src/model.rs:15`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L15)-`27`) as `{ default_id, styles: Vec<TableStyle> }`. A
`TableStyle` is a `style_id` plus the optional style parts that matter here -- `wholeTbl`,
`band1H`, `band2H`, `firstRow`, `lastRow`, `firstCol`, `lastCol` -- each holding a
`tcTxStyle` (bold/italic + a `ColorValue`) and a `tcStyle` (`a:fill` and the
`a:tcBdr` edges). It must be a parsed model field, not a lazy read of `part_bytes`, because
`parts` is `#[serde(skip)]` ([`crates/pptx-parse/src/model.rs:26`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L26)) and a collaboration-recovered
package has none.

Resolution order per cell, lowest to highest: `wholeTbl`, then the banding part
(`band1H`/`band2H`, alternating by data-row index when `bandRow="1"`), then `firstCol`/
`lastCol`, then `firstRow`/`lastRow`, then the cell's own `a:tcPr`. Colours resolve through
the existing theme path, so scheme colours in a style behave like any other.

When `style_id` names a style the package does not define -- 6 of the 23 tables in these
decks -- fall back to the `def` style id from `tableStyles.xml` before falling back to plain
`wholeTbl` defaults. That gets the grid and text right and leaves only the header hue wrong.
A built-in style table for the common PowerPoint GUIDs can follow later; do not block on it.

**3. Lay the table out (`crates/pptx-render/src/layout.rs`).** Replace the placeholder push
in `render_graphic_frame` ([`crates/pptx-render/src/layout.rs:625`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L625)) with a `render_table` arm
for `GraphicFrameData::Table`, keeping the placeholder as the fallback for `Diagram` and
`Unknown`:

- Column x offsets: scale `grid` so the widths sum to the frame's width (PowerPoint stores
  both, and they can disagree by rounding), then run a prefix sum.
- Row heights: start from `a:tr@h`, lay each cell's text out with the existing
  `layout_content`, and grow the row to `max(h, tallest cell content + insets)`. This is what
  produces LibreOffice's overflowing ocp-psp-plan/04 table; a first cut may skip it and take
  `a:tr@h` literally, which is within a few pixels on most rows here.
- Merges: a cell with `hMerge`/`vMerge` is a continuation and emits nothing; the origin cell
  with `gridSpan`/`rowSpan` spans the summed widths/heights. All 72 merged cells in the decks
  are in ocp-psp-plan.
- Emit one `Primitive::Table` per frame, built like `Primitive::Chart`
  ([`crates/pptx-render/src/display_list.rs:148`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L148)-`161`): a container whose children paint
  clipped to its rectangle. Children per cell: a `Primitive::Shape` with the resolved fill,
  a `Primitive::Shape` per drawn border edge, and the `Primitive::TextBox` that
  `render_text_box` ([`crates/pptx-render/src/layout.rs:657`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L657)) already produces -- give it the
  cell rect and a `BodyCascade` whose `primary` is the cell's `TextBody`. Draw all fills
  before all borders, or shared edges get overpainted.
- Cell text comes from the snapshot when present (`shape.text_stories[row * cols + col]` via
  `content_from_story`) and from the parsed cell body otherwise (`content_from_body`), which
  is what the layout/master path at [`crates/pptx-render/src/layout.rs:551`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L551) needs.
- Then fix the stray-text bug: [`crates/pptx-render/src/layout.rs:458`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L458) must not run
  `text_stories.first()` for `ShapeKind::GraphicFrame`, because `render_table` now consumes
  every story. This alone is a two-line change and can land first.
- `render_text_box` charges `self.line_count` against `MAX_TEXT_LINES`; a 1204-cell deck will
  push that budget, so re-check the limit before shipping.

**4. Paint it (`crates/pptx-raster`, `packages/pptx`).** `Primitive::Table` paints exactly
like `Primitive::Chart` today: clip to the rect, recurse over children
([`crates/pptx-raster/src/lib.rs:303`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L303)-`318`; `paintChart` in
[`packages/pptx/src/render/canvas.ts:92`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L92)-`101`). Add the variant to `SlidePrimitive`
([`packages/pptx/src/types.ts:331`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L331)-`336`) and a `case 'table'` at
[`packages/pptx/src/render/canvas.ts:80`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L80). Bump `CONTRACT_VERSION`
([`crates/pptx-render/src/display_list.rs:5`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L5)) since a new primitive kind reaches older
consumers as an unhandled `kind`.

[`crates/pptx-render/src/lib.rs:213`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L213) (`ComposedShape::TablePlaceholder`) is a separate,
host-fed compile path with no table payload; leave it emitting a placeholder.

```rust
// pptx-parse/src/model.rs
pub enum GraphicFrameData {
    Table(Table),
    // ... Chart / Diagram / Unknown unchanged
}

pub struct Table {
    pub grid: Vec<i64>,            // a:gridCol@w, EMU
    pub properties: TableProperties, // firstRow/bandRow/... + style_id
    pub rows: Vec<TableRow>,
}

pub struct TableRow { pub height: i64, pub cells: Vec<TableCell> }

pub struct TableCell {
    pub text: TextBody,            // a:txBody, with tcPr anchor/vert/mar* folded in
    pub grid_span: u32,
    pub row_span: u32,
    pub merged: bool,              // hMerge or vMerge: a continuation, draws nothing
    pub fill: Option<ShapeFill>,   // a:tcPr/a:solidFill
    pub borders: CellBorders,      // a:lnL / lnT / lnR / lnB
}

// pptx-render/src/layout.rs
fn render_table(&mut self, object_id: u32, shape_id: &str, name: &str, rect: PxRect,
                transform: Transform, table: &Table, stories: &[StorySnapshot])
    -> Result<(), RenderError>
{
    let style = self.package.table_styles.resolve(table.properties.style_id.as_deref());
    let xs = column_offsets(&table.grid, rect.w);       // prefix sum, scaled to rect.w
    let ys = row_offsets(self, table, &xs, rect)?;      // a:tr@h, grown to fit content
    let mut children = Vec::new();
    for (r, row) in table.rows.iter().enumerate() {
        for (c, cell) in row.cells.iter().enumerate() {
            if cell.merged { continue; }
            let cell_rect = span_rect(&xs, &ys, r, c, cell);
            let resolved = style.resolve_cell(&table.properties, r, c, table.rows.len(), cell);
            if let Some(paint) = resolved.fill { children.push(fill_shape(cell_rect, paint)); }
            // fills first, then borders, so shared edges are not overpainted
            children.extend(border_shapes(cell_rect, &resolved.borders));
            let content = stories.get(r * row.cells.len() + c)
                .map(content_from_story)
                .unwrap_or_else(|| content_from_body(shape_id, &cell.text, self.theme));
            children.push(self.cell_text(object_id, shape_id, cell_rect, transform,
                                         content, &cell.text, &resolved)?);
        }
    }
    self.primitives.push(Primitive::Table {
        object_id, shape_id: Some(shape_id.to_owned()), name: name.to_owned(),
        x: rect.x, y: rect.y, w: rect.w, h: rect.h,
        label: format!("Table, {} rows, {} columns", table.rows.len(), table.grid.len()),
        primitives: children, transform,
    });
    Ok(())
}
```

```ts
// packages/pptx/src/render/canvas.ts -- identical to paintChart
case 'table':
  await paintTable(ctx, primitive, options);
  break;
```

Risks and tests to add:

- **The style engine is the whole job for one deck.** rollout-plan has zero explicit cell
  fills and zero explicit borders; a fills-and-borders-from-`tcPr`-only first cut leaves its
  6 findings looking barely better than the placeholder (grid text on white). project20's 3
  tables, by contrast, are complete without any style resolution. Sequence the work so
  project20/05 validates layers 1, 3 and 4 before layer 2 lands.
- **Unresolvable built-in style ids.** 6 of 23 tables reference GUIDs absent from their
  `tableStyles.xml`. Whatever the fallback is, ocp-psp-plan/04 and /09 will keep a header
  colour that differs from LibreOffice's built-in rendering. Say so in the PR rather than
  chasing the diff.
- **Row auto-growth changes slide overflow.** Growing rows to fit content is what PowerPoint
  and LibreOffice do, and it makes ocp-psp-plan/04's table run off the bottom of the slide --
  which looks like a regression next to a tidy clipped table but matches the reference.
- **Text budget.** 1204 cells across three decks all route through `render_text_box`, which
  accumulates into `MAX_TEXT_LINES` ([`crates/pptx-render/src/layout.rs:699`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L699)-`704`). A single
  large table could now trip a resource limit that no slide reached before.
- **Contract version.** Adding a primitive kind breaks any consumer that exhaustively
  switches on `kind`. Bump `CONTRACT_VERSION` and update the fixtures at
  [`packages/pptx/src/render/canvas.test.ts:30`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.test.ts#L30), `:129` and
  [`packages/pptx/src/render/png.test.ts:6`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/packages/pptx/src/render/png.test.ts#L6).
- **Editing.** Cell stories already exist and are already addressable for writes
  ([`crates/pptx-parse/src/write.rs:1274`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1274)-`1278`), so text edits should round-trip -- but each
  cell now emits its own `TextBox` with its own `story_id`, which changes hit-testing for
  table frames. Check `HitRegion` handling for the frame before assuming the editor is
  unaffected.

Tests to add: a `parse_graphic_frame` unit test in `crates/pptx-parse/src/drawing.rs`
asserting grid widths, row heights, a `gridSpan` origin plus its `hMerge` continuation, and a
`tcPr` fill and border survive parsing; a `tableStyles.xml` test covering a `firstRow` +
`band1H` resolution and the missing-style-id fallback; a `pptx-render` layout test asserting
a 2x2 table emits one `Primitive::Table` whose children are 4 fills and 4 text boxes at the
expected rects; and a `golden_table` case in `crates/pptx-raster/tests/golden.rs` alongside
the existing `golden_placeholder` (`:327`), which should stay to cover `Diagram`/`Unknown`.

**How to verify**

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
[`crates/pptx-raster/tests/golden.rs:327`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/tests/golden.rs#L327)-`343` (`golden_placeholder`, which renders exactly
this "Table" placeholder into `crates/pptx-raster/tests/golden/placeholder.png` and can stay
as the fallback case) and the chart-frame test at
[`crates/pptx-render/src/layout.rs:2379`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2379), which asserts a graphic frame without its part keeps
a placeholder. Nothing in `crates/pptx-parse` asserts anything about `a:tbl` beyond the
text-target lookup in [`crates/pptx-parse/src/write.rs:1274`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1274)-`1278`, so the fix needs new
tests at every layer.

**Additional context**

none.

Related issues found in the same run: none.

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/package.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/display_list.rs`, `crates/pptx-render/src/lib.rs`, `crates/pptx-raster/src/lib.rs`, `crates/pptx-edit/src/deck.rs`, `packages/pptx/src/render/canvas.ts`, `packages/pptx/src/types.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/unsupported-table-not-rendered/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
