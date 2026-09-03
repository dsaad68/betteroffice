# Possible solution: geometry-custom-collapses-to-bbox

## Approach

Parse `a:custGeom/a:pathLst` in `pptx-parse` into the shared
`ooxml_drawingml::GeometryPathCommand` vocabulary, normalised to the same 0..1 unit space the
preset table already emits, hang it off `Shape`, and have the three layout emit sites prefer it over
`preset_geometry_to_path`. Nothing below the display list changes: `Primitive::Shape.path`
(`crates/pptx-render/src/display_list.rs:89`), `crates/pptx-raster/src/lib.rs:575` and
`packages/pptx/src/render/canvas.ts:118` already draw arbitrary command lists, and
`crates/pptx-render/src/chart.rs:155` already ships `geometry: "custom"` primitives through them.

Four edits, in dependency order:

1. **`crates/pptx-parse/src/drawing.rs`** — port `parse_custom_geometry_path` /
   `parse_custom_path` / `normalize_raw_path` from `crates/docx-parse/src/drawingml.rs:276-527`,
   swapping `docx_parse::xml::XmlElement` calls (`child_by_full_name`, `children_by_local_name`,
   `local_child`) for the `pptx_parse::xml::XmlElement` equivalents already used in this file
   (`child`, `child_elements`, `local_name`). Resolve point coordinates through the existing
   `evaluate_guide_formula` (`crates/pptx-parse/src/drawing.rs:445`) seeded with `gdLst` and the
   path's own `w`/`h`, falling back to a plain numeric parse — the corpus needs only the numeric
   branch, but the guide branch is what makes the port correct for other decks. Keep the
   `MAX_CUSTOM_PATH_COMMANDS` / `MAX_CUSTOM_GUIDES` caps; charge nothing extra against
   `ParseBudget`, which already bounds XML events upstream.

   Prefer moving the shared half (`normalize_raw_path`, `arc_to_cubics`, `standard_guide_values`,
   the command builder) into `crates/ooxml-drawingml` behind a tiny element-accessor trait, so
   `docx-parse` and `pptx-parse` stop carrying two copies. Duplicating into `pptx-parse` is the
   cheaper first cut; say which one the PR takes.

2. **`crates/pptx-parse/src/model.rs`** — add
   `pub geometry_path: Option<Vec<GeometryPathCommand>>` to `Shape` (beside `geometry` at
   `crates/pptx-parse/src/model.rs:201`), `#[serde(default, skip_serializing_if = "Option::is_none")]`
   so existing serialised models still deserialise.

3. **`crates/pptx-render`** — thread it through the two `geometry_path(...)` calls at
   `crates/pptx-render/src/layout.rs:410` and `crates/pptx-render/src/layout.rs:516`, and add an
   optional `path` to `ComposedShape::Shape` for `crates/pptx-render/src/lib.rs:170`. Keep the
   rect fallback for a `custGeom` that parses to nothing.

4. **`crates/pptx-edit`** — store the commands as `geometryPathJson` next to `adjustValuesJson`
   (`crates/pptx-edit/src/deck.rs:126-129`), add the field to `ShapeSnapshot`
   (`crates/pptx-edit/src/model.rs:111`) and read it back at `crates/pptx-edit/src/deck.rs:816`.
   That is a document-shape change, so bump `SCHEMA_VERSION` 2.0 -> 3.0
   (`crates/pptx-edit/src/deck.rs:23`) and extend `MIGRATABLE_SCHEMA_VERSIONS`
   (`crates/pptx-edit/src/deck.rs:25`); a v2 document simply has no path key and keeps rendering as
   it does today.

`crates/pptx-parse/src/write.rs:1716` needs no change — it only refuses to *author* a new `custom`
shape, and writing is part-preserving, so existing `custGeom` XML round-trips untouched.

## Sketch

```rust
// crates/pptx-parse/src/drawing.rs
fn parse_custom_geometry_path(geometry: Option<&XmlElement>) -> Option<Vec<GeometryPathCommand>> {
    let path = geometry?.child("pathLst")?.child("path")?; // corpus: always exactly one
    let width = numeric_attribute(Some(path), "w")? as f64;
    let height = numeric_attribute(Some(path), "h")? as f64;
    let guides = custom_guide_values(geometry, width, height); // gdLst via evaluate_guide_formula
    let mut out = Vec::new();
    for child in path.child_elements() {
        if out.len() >= MAX_CUSTOM_PATH_COMMANDS { break; }
        match child.local_name() {
            "moveTo" => push_point(&mut out, child, &guides, GeometryPathCommand::Move),
            "lnTo" => push_point(&mut out, child, &guides, GeometryPathCommand::Line),
            "cubicBezTo" => { /* 3 pts */ }
            "quadBezTo" => { /* 2 pts */ }
            "arcTo" => { /* port arc_to_cubics */ }
            "close" => out.push(GeometryPathCommand::Close),
            _ => {}
        }
    }
    // divide every coordinate by the path's own w/h -> the 0..1 space the presets emit
    (!out.is_empty()).then(|| normalize(out, width, height))
}

fn parse_geometry_path(properties: Option<&XmlElement>) -> Option<Vec<GeometryPathCommand>> {
    parse_custom_geometry_path(properties.and_then(|value| value.child("custGeom")))
}

// crates/pptx-render/src/layout.rs (both emit sites)
path: shape
    .geometry_path
    .clone()
    .unwrap_or_else(|| geometry_path(&shape.geometry, &shape.adjust_values, aspect_ratio)),
```

## Risks

- **Display-list size.** `cisco-cloud-security/04` and `/19` emit 131 custom shapes / 18,569
  commands; `project17/11` emits 388 / 10,378. Roughly a megabyte of extra JSON per slide crosses
  the wasm boundary and, for the snapshot path, lands in the Yjs document. If that bites, store the
  commands once in the parsed model and let the snapshot reference them rather than copying, or add
  a decimation pass for near-collinear runs. Measure before optimising.
- **Winding rule.** `crates/pptx-raster/src/lib.rs:376-386` fills with `FillRule::Winding`. Donut
  and keyhole shapes with a reversed inner loop rely on that; a shape drawn with two same-direction
  loops will fill solid. Add a golden for one (`ocp-psp-plan/01`'s ring, or the padlock on
  `cisco-cloud-security/05`).
- **Unclosed paths.** Many corpus paths end without `<a:close/>`. Filling an open path is defined
  (implicit close) but stroking it is not — `crates/pptx-raster/src/lib.rs:482` must not draw a
  closing segment where PowerPoint leaves the outline open. `project20/16/3` is exactly this case
  and is the cheapest slide to check it on.
- **`pptx-edit` schema bump.** Bumping `SCHEMA_VERSION` touches every persisted document; keep the
  v2 migration path and cover it with the existing migration test around
  `crates/pptx-edit/src/deck.rs:673`.
- **No regression on charts.** `crates/pptx-render/src/chart.rs:485` and
  `crates/pptx-render/src/chart.rs:594` assert on `geometry == "custom"` primitives produced by the
  chart path; the shape-side change must leave them alone.

## Effort

**Medium.** The hard parts are already solved and testable prior art
(`crates/docx-parse/src/drawingml.rs:276-527`), the display list, raster and canvas backends need no
change at all, and the corpus needs only `moveTo`/`lnTo`/`cubicBezTo`/`close` with literal
coordinates — the bulk of the work is the `XmlElement` API translation plus the `pptx-edit` schema
bump and migration. It would be easy if the snapshot path did not need a new field.
