# pptx: Line arrowheads are parsed and never drawn

**Describe the bug**

A line that ends in an arrow is drawn as a bare line. `a:headEnd` and `a:tailEnd` are read off
`a:ln` and stored, and then nothing anywhere consumes them: the display list's stroke carries a
colour, a width and a boolean for dashed, and has no room for an end cap.

This was found by eye on `project20/04` rather than by the comparator, which recorded the whole
connector as missing and attributed it to `line-zero-extent-skipped`. That cluster explains the
line; it does not explain the arrowhead, which stays missing after connectors parse.

Seen on 18 slides across 3 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project20/04** `Straight Arrow Connector 87`, a `sysDot` line with `tailEnd type="triangle"`, in the status row. The reference draws a dotted green line ending in a solid triangle; the candidate draws neither.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-arrowheads-not-drawn/evidence-1.png)

**2. cisco-cloud-security/11** Six connectors in the same diagram, each with `tailEnd type="triangle"`, none drawn.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-arrowheads-not-drawn/evidence-2.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 4, 7, 11, 19
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 4
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 4

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

`parse_outline` reads both ends into `ShapeOutline`
([`crates/pptx-parse/src/drawing.rs:641`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L641)-`642`, into the fields declared at
`crates/ooxml-drawingml/src/shape.rs`), and a search across `pptx-render`, `pptx-raster` and
`packages/pptx` for `head_end` or `tail_end` returns nothing. The value dies at the boundary
between the parsed outline and the display list.

`stroke()` (`crates/pptx-render/src/layout.rs`) builds a `Stroke` from the outline and copies
only three fields, and `Stroke` itself (`crates/pptx-render/src/display_list.rs`) is
`{ color, width, dashed }`. Neither backend could draw an arrowhead even if it wanted to:
`paint_shape` in `crates/pptx-raster/src/lib.rs` strokes the path and stops, and
`packages/pptx/src/render/canvas.ts` does the same.

Counted across the twelve sample decks: 18 shapes carry a non-`none` end, 11 of them connectors
and 7 plain shapes.

**Suggested fix**

Carry the two ends onto the display-list stroke and draw them in the backends.

1. Add an optional end description to `Stroke` in `crates/pptx-render/src/display_list.rs`,
   defaulted and skipped when absent so the contract stays additive and existing output is
   unchanged.
2. Populate it in `stroke()` from `ShapeOutline::head_end` and `tail_end`, ignoring
   `type="none"` so the common case adds nothing.
3. In `crates/pptx-raster/src/lib.rs`, after stroking the path, build the arrow as a filled
   triangle at the first and last point of the path, oriented along that segment, scaled from
   the stroke width by the `width` and `length` attributes.
4. Mirror the field in `packages/pptx/src/types.ts` and draw the same triangle in
   `packages/pptx/src/render/canvas.ts`.

```rust
// display_list.rs
pub struct Stroke {
    pub color: String,
    pub width: f32,
    pub dashed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_end: Option<LineEndMark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_end: Option<LineEndMark>,
}
```

Risks and tests to add:

- Only `triangle`, `arrow`, `oval` and `stealth` appear in the sample decks. Anything else should
  fall through to no mark rather than guess.
- The mark must be drawn in the shape's own space, so it inherits the transform. A rotated or
  flipped connector otherwise points the wrong way.
- Nothing renders until `line-zero-extent-skipped` merges, since every observable case is a
  connector. Do not read a flat diff as failure.

**How to verify**

Re-render `project20/04` and `cisco-cloud-security/11` with the connector fix also applied. The
green dotted line in the status row should end in a filled triangle, and the six connectors in
the Cisco diagram should each gain one. The pixel difference will barely move: an arrowhead is a
few dozen pixels. Judge it on the crops.

**Additional context**

*Not confirmed*

The 7 plain shapes are all `custGeom`, which currently draws as its bounding rectangle, so their
arrowheads cannot be judged until `geometry-custom-collapses-to-bbox` lands. Every observable
case is therefore a connector, which means this issue produces no visible change until
`line-zero-extent-skipped` is merged.

Adjacent and deliberately excluded: `a:prstDash` is parsed into `ShapeOutline::style` but the
stroke reduces it to one boolean, so `sysDot` and `dash` render identically. That is the same
plumbing but different work, and it wants its own change.

Related issues found in the same run: `geometry-custom-collapses-to-bbox`, #272

Files most likely involved: `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/display_list.rs`, `crates/pptx-raster/src/lib.rs`, `packages/pptx/src/types.ts`, `packages/pptx/src/render/canvas.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 36 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/line-arrowheads-not-drawn/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
