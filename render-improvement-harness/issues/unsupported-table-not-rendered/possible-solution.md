# Possible solution: unsupported-table-not-rendered

## Approach

Four layers, in dependency order. Each is separately shippable and each visibly improves the
harness decks, so this should land as a stack rather than one change.

**1. Parse the table (`crates/pptx-parse`).** Widen `GraphicFrameData::Table`
(`crates/pptx-parse/src/model.rs:243`-`245`) from `rows: Vec<Vec<TextBody>>` to a real
`Table` struct: `grid: Vec<i64>` from `a:tblGrid/a:gridCol@w`, `properties` from `a:tblPr`
(`first_row`, `first_col`, `last_row`, `last_col`, `band_row`, `band_col`, `style_id`), and
`rows: Vec<TableRow>` where a row carries `height: i64` (`a:tr@h`) and
`cells: Vec<TableCell>`. A `TableCell` carries the existing `TextBody`, the merge attributes
(`grid_span`, `row_span`, `h_merge`, `v_merge`), the `a:tcPr` fill (`parse_fill`,
`crates/pptx-parse/src/drawing.rs:565`) and four optional borders. Borders need a small
variant of `parse_outline` (`crates/pptx-parse/src/drawing.rs:624`) that takes the
`a:lnL`/`lnT`/`lnR`/`lnB` element itself rather than looking up a child named `ln`.

Fold `a:tcPr`'s `anchor`, `vert` and `marL`/`marT`/`marR`/`marB` straight into the cell's
`TextBody` (`anchor`, `vertical`, `inset_*` -- `crates/pptx-parse/src/model.rs:266`-`278`) at
parse time. That is exactly the shape `BodyCascade` already reads
(`crates/pptx-render/src/layout.rs:769`-`780`), so cell text needs no new cascade plumbing.

Keep `rows` row-major and the cell order stable: `seed_shape` derives story ids positionally
as `story:{shape}:table:{row}:{cell}` (`crates/pptx-edit/src/deck.rs:152`-`158`), and layout
will zip stories against cells by the same index.

**2. Parse table styles (`crates/pptx-parse/src/package.rs`).** Add a `table_styles` stage
next to the theme stage in `parse_pptx_with_limits` (`crates/pptx-parse/src/package.rs:17`),
reading `ppt/tableStyles.xml` into `PptxPackage.table_styles`
(`crates/pptx-parse/src/model.rs:15`-`27`) as `{ default_id, styles: Vec<TableStyle> }`. A
`TableStyle` is a `style_id` plus the optional style parts that matter here -- `wholeTbl`,
`band1H`, `band2H`, `firstRow`, `lastRow`, `firstCol`, `lastCol` -- each holding a
`tcTxStyle` (bold/italic + a `ColorValue`) and a `tcStyle` (`a:fill` and the
`a:tcBdr` edges). It must be a parsed model field, not a lazy read of `part_bytes`, because
`parts` is `#[serde(skip)]` (`crates/pptx-parse/src/model.rs:26`) and a collaboration-recovered
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
in `render_graphic_frame` (`crates/pptx-render/src/layout.rs:625`) with a `render_table` arm
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
  (`crates/pptx-render/src/display_list.rs:148`-`161`): a container whose children paint
  clipped to its rectangle. Children per cell: a `Primitive::Shape` with the resolved fill,
  a `Primitive::Shape` per drawn border edge, and the `Primitive::TextBox` that
  `render_text_box` (`crates/pptx-render/src/layout.rs:657`) already produces -- give it the
  cell rect and a `BodyCascade` whose `primary` is the cell's `TextBody`. Draw all fills
  before all borders, or shared edges get overpainted.
- Cell text comes from the snapshot when present (`shape.text_stories[row * cols + col]` via
  `content_from_story`) and from the parsed cell body otherwise (`content_from_body`), which
  is what the layout/master path at `crates/pptx-render/src/layout.rs:551` needs.
- Then fix the stray-text bug: `crates/pptx-render/src/layout.rs:458` must not run
  `text_stories.first()` for `ShapeKind::GraphicFrame`, because `render_table` now consumes
  every story. This alone is a two-line change and can land first.
- `render_text_box` charges `self.line_count` against `MAX_TEXT_LINES`; a 1204-cell deck will
  push that budget, so re-check the limit before shipping.

**4. Paint it (`crates/pptx-raster`, `packages/pptx`).** `Primitive::Table` paints exactly
like `Primitive::Chart` today: clip to the rect, recurse over children
(`crates/pptx-raster/src/lib.rs:303`-`318`; `paintChart` in
`packages/pptx/src/render/canvas.ts:92`-`101`). Add the variant to `SlidePrimitive`
(`packages/pptx/src/types.ts:331`-`336`) and a `case 'table'` at
`packages/pptx/src/render/canvas.ts:80`. Bump `CONTRACT_VERSION`
(`crates/pptx-render/src/display_list.rs:5`) since a new primitive kind reaches older
consumers as an unhandled `kind`.

`crates/pptx-render/src/lib.rs:213` (`ComposedShape::TablePlaceholder`) is a separate,
host-fed compile path with no table payload; leave it emitting a placeholder.

## Sketch

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

## Risks

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
  accumulates into `MAX_TEXT_LINES` (`crates/pptx-render/src/layout.rs:699`-`704`). A single
  large table could now trip a resource limit that no slide reached before.
- **Contract version.** Adding a primitive kind breaks any consumer that exhaustively
  switches on `kind`. Bump `CONTRACT_VERSION` and update the fixtures at
  `packages/pptx/src/render/canvas.test.ts:30`, `:129` and
  `packages/pptx/src/render/png.test.ts:6`.
- **Editing.** Cell stories already exist and are already addressable for writes
  (`crates/pptx-parse/src/write.rs:1274`-`1278`), so text edits should round-trip -- but each
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

## Effort

Hard. It spans four crates plus the TypeScript renderer, adds a model type, a package part, a
display-list primitive and a contract bump, and the table-style cascade is a real feature in
its own right -- one deck in this cluster is unreadable without it. Layer 3's stray-text fix
(`crates/pptx-render/src/layout.rs:458`) is the only easy piece and is worth landing alone.
