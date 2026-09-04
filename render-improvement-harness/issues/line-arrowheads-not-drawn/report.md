---
id: line-arrowheads-not-drawn
title: Line arrowheads are parsed and never drawn
category: line
impact: medium
effort: medium
confidence: high
status: open
occurrences: 18
decks: [cisco-cloud-security, project17, project20]
findings: [project20/04/a1, cisco-cloud-security/11/a1, cisco-cloud-security/07/a1, cisco-cloud-security/04/a1, cisco-cloud-security/19/a1, project17/04/a1]
files: [crates/pptx-render/src/layout.rs, crates/pptx-render/src/display_list.rs, crates/pptx-raster/src/lib.rs, packages/pptx/src/types.ts, packages/pptx/src/render/canvas.ts]
---

## Symptom

A line that ends in an arrow is drawn as a bare line. `a:headEnd` and `a:tailEnd` are read off
`a:ln` and stored, and then nothing anywhere consumes them: the display list's stroke carries a
colour, a width and a boolean for dashed, and has no room for an end cap.

This was found by eye on `project20/04` rather than by the comparator, which recorded the whole
connector as missing and attributed it to `line-zero-extent-skipped`. That cluster explains the
line; it does not explain the arrowhead, which stays missing after connectors parse.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project20/04 | `Straight Arrow Connector 87`, a `sysDot` line with `tailEnd type="triangle"`, in the status row. The reference draws a dotted green line ending in a solid triangle; the candidate draws neither. |
| 2 | cisco-cloud-security/11 | Six connectors in the same diagram, each with `tailEnd type="triangle"`, none drawn. |

## Root cause (confirmed)

`parse_outline` reads both ends into `ShapeOutline`
(`crates/pptx-parse/src/drawing.rs:641`-`642`, into the fields declared at
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

## Not confirmed

The 7 plain shapes are all `custGeom`, which currently draws as its bounding rectangle, so their
arrowheads cannot be judged until `geometry-custom-collapses-to-bbox` lands. Every observable
case is therefore a connector, which means this issue produces no visible change until
`line-zero-extent-skipped` is merged.

Adjacent and deliberately excluded: `a:prstDash` is parsed into `ShapeOutline::style` but the
stroke reduces it to one boolean, so `sysDot` and `dash` render identically. That is the same
plumbing but different work, and it wants its own change.

## Verification

Re-render `project20/04` and `cisco-cloud-security/11` with the connector fix also applied. The
green dotted line in the status row should end in a filled triangle, and the six connectors in
the Cisco diagram should each gain one. The pixel difference will barely move: an arrowhead is a
few dozen pixels. Judge it on the crops.
