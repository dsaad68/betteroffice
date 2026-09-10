use pptx_parse::{ParseLimits, PptxError, parse_pptx_with_limits};

const SLIDE_REFERENCED_TWICE: &[u8] =
    include_bytes!("fixtures/fuzz/slide-referenced-twice.records");

/// The limits the `pptx-package-parse` fuzz target parses under.
fn fuzz_limits() -> ParseLimits {
    ParseLimits {
        max_xml_bytes: 4 << 20,
        max_xml_events: 200_000,
        max_xml_text_bytes: 4 << 20,
        max_xml_depth: 64,
        max_attributes_per_element: 128,
        max_attribute_bytes: 64 << 10,
        max_relationships: 64,
        max_shapes: 96,
        max_paragraphs: 96,
        max_runs: 128,
        max_comments: 16,
    }
}

/// Zips the fuzz target's NUL-separated `path\ncontent` records.
fn package(records: &[u8]) -> Vec<u8> {
    let parts = records
        .split(|byte| *byte == 0)
        .map(|record| {
            let split = record
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap_or(record.len());
            let (path, content) = record.split_at(split);
            (
                String::from_utf8_lossy(path).into_owned(),
                content.get(1..).unwrap_or_default().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    ooxml_opc::rezip_parts(&parts).unwrap()
}

#[test]
fn a_slide_referenced_twice_spends_one_package_shape_budget() {
    assert!(matches!(
        parse_pptx_with_limits(&package(SLIDE_REFERENCED_TWICE), &fuzz_limits()),
        Err(PptxError::ResourceLimit { kind: "shapes", .. })
    ));
}
