use std::collections::BTreeMap;
use std::io::Cursor;

use docx_parse::{S9ParseOptions, parse_docx_s9_wire};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
use quick_xml::Reader;
use quick_xml::events::Event;

use super::*;

const DOCX_SECRETS: &[&str] = &[
    "DOCX_SECRET_TEXT",
    "DOCX_SECRET_COMMENT",
    "DOCX_SECRET_AUTHOR",
    "DOCX_SECRET_TITLE",
    "DOCX_SECRET_COMPANY",
    "https://secret.example/docx",
    "DOCX_SECRET_IDENTIFIER",
    "DOCX_SECRET_LANGUAGE",
    "DOCX_SECRET_VERSION",
    "DOCX_SECRET_CONTENT_TYPE",
];
const XLSX_SECRETS: &[&str] = &[
    "XLSX_SECRET_TEXT",
    "XLSX_INLINE_SECRET",
    "XLSX_SECRET_SHEET",
    "XLSX_SECRET_AUTHOR",
    "XLSX_SECRET_COMPANY",
    "https://secret.example/xlsx",
];
const PPTX_SECRETS: &[&str] = &[
    "PPTX_SECRET_TEXT",
    "PPTX_SECRET_NOTES",
    "PPTX_SECRET_AUTHOR",
    "PPTX_SECRET_COMPANY",
    "https://secret.example/pptx",
];

#[test]
fn redacts_docx_without_changing_structure() {
    let source = docx_fixture();
    let (output, report) = redact_with_report(&source, Format::Auto).unwrap();
    assert_eq!(report.format, Format::Docx);
    assert_fixture_properties(&source, &output, DOCX_SECRETS, "word/media/image1.png");
    assert_text_lengths(&source, &output, "word/document.xml", "t");
    parse_docx_s9_wire(&output, S9ParseOptions::default()).unwrap();
}

#[test]
fn redacts_xlsx_without_changing_structure() {
    let source = xlsx_fixture();
    let (output, report) = redact_with_report(&source, Format::Xlsx).unwrap();
    assert_eq!(report.format, Format::Xlsx);
    assert_fixture_properties(&source, &output, XLSX_SECRETS, "xl/media/image1.png");
    assert_text_lengths(&source, &output, "xl/sharedStrings.xml", "t");
    let parts = ooxml_opc::unzip_parts(&output).unwrap();
    xlsx_parse::parse_workbook(&parts).unwrap();
}

#[test]
fn redacts_pptx_without_changing_structure() {
    let source = pptx_fixture();
    let (output, report) = redact_with_report(&source, Format::Pptx).unwrap();
    assert_eq!(report.format, Format::Pptx);
    assert_fixture_properties(&source, &output, PPTX_SECRETS, "ppt/media/image1.png");
    assert_text_lengths(&source, &output, "ppt/slides/slide1.xml", "t");
    pptx_parse::parse_pptx(&output).unwrap();
}

#[test]
fn preserves_jpeg_dimensions_and_format() {
    let source = placeholder_image(ImageFormat::Jpeg);
    let mut report = RedactionReport::default();
    let output = media::replace_media("word/media/photo.jpeg", &source, &mut report).unwrap();
    assert_ne!(source, output);
    assert_eq!(image_dimensions(&source), image_dimensions(&output));
    assert_eq!(image::guess_format(&output).unwrap(), ImageFormat::Jpeg);
}

#[test]
fn rejects_explicit_format_mismatch() {
    let error = redact(&docx_fixture(), Format::Xlsx).unwrap_err();
    assert!(matches!(error, RedactError::FormatMismatch { .. }));
}

fn assert_fixture_properties(source: &[u8], output: &[u8], secrets: &[&str], media_path: &str) {
    let before = ooxml_opc::unzip_parts(source).unwrap();
    let after = ooxml_opc::unzip_parts(output).unwrap();
    assert_eq!(part_names(&before), part_names(&after));
    assert_eq!(element_counts(&before), element_counts(&after));

    for secret in secrets {
        assert!(
            after
                .iter()
                .all(|(_, bytes)| !String::from_utf8_lossy(bytes).contains(secret)),
            "secret survived: {secret}"
        );
    }

    let before_image = part(&before, media_path);
    let after_image = part(&after, media_path);
    assert_ne!(before_image, after_image);
    assert_eq!(
        image_dimensions(before_image),
        image_dimensions(after_image)
    );
    assert_eq!(image::guess_format(after_image).unwrap(), ImageFormat::Png);
}

fn part_names(parts: &[(String, Vec<u8>)]) -> Vec<&str> {
    parts.iter().map(|(path, _)| path.as_str()).collect()
}

fn element_counts(parts: &[(String, Vec<u8>)]) -> BTreeMap<&str, usize> {
    parts
        .iter()
        .filter(|(path, _)| is_xml_part(&path.to_ascii_lowercase()))
        .map(|(path, bytes)| (path.as_str(), element_count(bytes)))
        .collect()
}

fn element_count(bytes: &[u8]) -> usize {
    let mut reader = Reader::from_reader(bytes);
    let mut count = 0;
    loop {
        match reader.read_event().unwrap() {
            Event::Start(_) | Event::Empty(_) => count += 1,
            Event::Eof => return count,
            _ => {}
        }
    }
}

fn assert_text_lengths(source: &[u8], output: &[u8], path: &str, element: &str) {
    let before = ooxml_opc::unzip_parts(source).unwrap();
    let after = ooxml_opc::unzip_parts(output).unwrap();
    assert_eq!(
        text_lengths(part(&before, path), element),
        text_lengths(part(&after, path), element)
    );
}

fn text_lengths(bytes: &[u8], target: &str) -> Vec<usize> {
    let mut reader = Reader::from_reader(bytes);
    let mut inside = false;
    let mut lengths = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(start) if start.name().local_name().as_ref() == target.as_bytes() => {
                inside = true;
            }
            Event::Text(text) if inside => lengths.push(text.decode().unwrap().chars().count()),
            Event::End(end) if end.name().local_name().as_ref() == target.as_bytes() => {
                inside = false;
            }
            Event::Eof => return lengths,
            _ => {}
        }
    }
}

fn part<'a>(parts: &'a [(String, Vec<u8>)], path: &str) -> &'a [u8] {
    parts
        .iter()
        .find(|(candidate, _)| candidate == path)
        .map(|(_, bytes)| bytes.as_slice())
        .unwrap()
}

fn image_dimensions(bytes: &[u8]) -> (u32, u32) {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .unwrap()
        .into_dimensions()
        .unwrap()
}

fn placeholder_png() -> Vec<u8> {
    placeholder_image(ImageFormat::Png)
}

fn placeholder_image(format: ImageFormat) -> Vec<u8> {
    let image = DynamicImage::ImageRgb8(ImageBuffer::from_fn(3, 2, |x, y| {
        Rgb([(x * 80) as u8, (y * 100) as u8, 40])
    }));
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, format).unwrap();
    output.into_inner()
}

fn package(mut parts: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let owned: Vec<_> = parts
        .drain(..)
        .map(|(path, bytes)| (path.to_owned(), bytes))
        .collect();
    ooxml_opc::rezip_parts(&owned).unwrap()
}

fn xml(value: &str) -> Vec<u8> {
    value.as_bytes().to_vec()
}

fn docx_fixture() -> Vec<u8> {
    package(vec![
        (
            "[Content_Types].xml",
            xml(
                r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
        ),
        (
            "docProps/core.xml",
            xml(
                r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>DOCX_SECRET_TITLE</dc:title><dc:creator>DOCX_SECRET_AUTHOR</dc:creator><dc:identifier>DOCX_SECRET_IDENTIFIER</dc:identifier><dc:language>DOCX_SECRET_LANGUAGE</dc:language><cp:version>DOCX_SECRET_VERSION</cp:version><cp:contentType>DOCX_SECRET_CONTENT_TYPE</cp:contentType></cp:coreProperties>"#,
            ),
        ),
        (
            "docProps/app.xml",
            xml(
                r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Company>DOCX_SECRET_COMPANY</Company><Pages>1</Pages></Properties>"#,
            ),
        ),
        (
            "word/document.xml",
            xml(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>DOCX_SECRET_TEXT</w:t></w:r><w:ins w:id="1" w:author="DOCX_SECRET_AUTHOR"><w:r><w:t>tracked secret</w:t></w:r></w:ins><w:hyperlink r:id="rId9"><w:r><w:t>private link</w:t></w:r></w:hyperlink></w:p><w:sectPr/></w:body></w:document>"#,
            ),
        ),
        (
            "word/comments.xml",
            xml(
                r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="0" w:author="DOCX_SECRET_AUTHOR"><w:p><w:r><w:t>DOCX_SECRET_COMMENT</w:t></w:r></w:p></w:comment></w:comments>"#,
            ),
        ),
        (
            "word/_rels/document.xml.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://secret.example/docx" TargetMode="External"/><Relationship Id="rId10" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/></Relationships>"#,
            ),
        ),
        ("word/media/image1.png", placeholder_png()),
    ])
}

fn xlsx_fixture() -> Vec<u8> {
    package(vec![
        (
            "[Content_Types].xml",
            xml(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
        ),
        (
            "xl/workbook.xml",
            xml(
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="XLSX_SECRET_SHEET" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
            ),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
        ),
        (
            "xl/sharedStrings.xml",
            xml(
                r#"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="1" uniqueCount="1"><si><t>XLSX_SECRET_TEXT</t></si></sst>"#,
            ),
        ),
        (
            "xl/worksheets/sheet1.xml",
            xml(
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="inlineStr"><is><t>XLSX_INLINE_SECRET</t></is></c><c r="C1"><f>SUM(1,2)</f><v>3</v></c></row></sheetData></worksheet>"#,
            ),
        ),
        (
            "xl/comments1.xml",
            xml(
                r#"<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><authors><author>XLSX_SECRET_AUTHOR</author></authors><commentList/></comments>"#,
            ),
        ),
        (
            "xl/worksheets/_rels/sheet1.xml.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://secret.example/xlsx" TargetMode="External"/></Relationships>"#,
            ),
        ),
        (
            "docProps/app.xml",
            xml(
                r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Company>XLSX_SECRET_COMPANY</Company></Properties>"#,
            ),
        ),
        ("xl/media/image1.png", placeholder_png()),
    ])
}

fn pptx_fixture() -> Vec<u8> {
    package(vec![
        (
            "[Content_Types].xml",
            xml(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#,
            ),
        ),
        (
            "ppt/presentation.xml",
            xml(
                r#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/></p:presentation>"#,
            ),
        ),
        (
            "ppt/_rels/presentation.xml.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/></Relationships>"#,
            ),
        ),
        (
            "ppt/slides/slide1.xml",
            xml(
                r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld name="Private slide"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name="Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="PPTX secret box"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>PPTX_SECRET_TEXT</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#,
            ),
        ),
        (
            "ppt/notesSlides/notesSlide1.xml",
            xml(
                r#"<p:notes xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>PPTX_SECRET_NOTES</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>"#,
            ),
        ),
        (
            "ppt/commentAuthors.xml",
            xml(
                r#"<p:cmAuthorLst xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cmAuthor id="0" name="PPTX_SECRET_AUTHOR" initials="PSA"/></p:cmAuthorLst>"#,
            ),
        ),
        (
            "ppt/slides/_rels/slide1.xml.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://secret.example/pptx" TargetMode="External"/></Relationships>"#,
            ),
        ),
        (
            "docProps/app.xml",
            xml(
                r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Company>PPTX_SECRET_COMPANY</Company></Properties>"#,
            ),
        ),
        ("ppt/media/image1.png", placeholder_png()),
    ])
}

#[test]
fn pivot_cache_values_are_scrubbed() {
    let definition = r##"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cacheFields count="5"><cacheField name="Customer"><sharedItems count="1"><s v="PIVOT_CACHE_SECRET"/></sharedItems></cacheField><cacheField name="Amount"><sharedItems containsString="0" containsNumber="1" count="1"><n v="89.45"/></sharedItems></cacheField><cacheField name="Active"><sharedItems containsSemiMixedTypes="0" containsString="0" containsNumber="1" containsInteger="1" count="1"><b v="1"/></sharedItems></cacheField><cacheField name="Ordered"><sharedItems containsNonDate="0" containsDate="1" containsString="0" count="1"><d v="2024-05-06T07:08:09Z"/></sharedItems></cacheField><cacheField name="Ratio"><sharedItems count="1"><e v="#DIV/0!"/></sharedItems></cacheField></cacheFields></pivotCacheDefinition>"##;
    let records = r##"<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="1"><r><x v="0"/><n v="12.5"/><b v="1"/><d v="2024-05-06T07:08:09Z"/><e v="#DIV/0!"/><s v="PIVOT_RECORD_SECRET"/></r></pivotCacheRecords>"##;
    let definition_out = scrub(
        Format::Xlsx,
        "xl/pivotCache/pivotCacheDefinition1.xml",
        definition.as_bytes(),
    );
    let records_out = scrub(
        Format::Xlsx,
        "xl/pivotCache/pivotCacheRecords1.xml",
        records.as_bytes(),
    );
    assert!(!definition_out.contains("PIVOT_CACHE_SECRET"));
    assert!(!records_out.contains("PIVOT_RECORD_SECRET"));
    for leaked in ["89.45", "2024-05-06", "#DIV/0!"] {
        assert!(
            !definition_out.contains(leaked) && !records_out.contains(leaked),
            "{leaked} survived"
        );
    }
    assert!(definition_out.contains(r#"<s v="xxxxxxxxxxxxxxxxxx"/>"#));
    assert!(records_out.contains(r#"<s v="xxxxxxxxxxxxxxxxxxx"/>"#));
    for typed in [
        r#"<n v="0"/>"#,
        r#"<b v="0"/>"#,
        r#"<d v="1970-01-01T00:00:00Z"/>"#,
        r##"<e v="#N/A"/>"##,
    ] {
        assert!(definition_out.contains(typed), "{typed} missing");
        assert!(records_out.contains(typed), "{typed} missing");
    }
    assert!(records_out.contains(r#"<x v="0"/>"#), "index was rewritten");
    assert_eq!(definition_out.matches("count=\"1\"").count(), 5);
}

#[test]
fn threaded_comment_text_and_person_identity_are_scrubbed() {
    let comments = r#"<threadedComments xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><threadedComment ref="A1" dT="2024-01-01T00:00:00Z" id="{11111111-1111-1111-1111-111111111111}" personId="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}"><text>THREAD_SECRET_TEXT</text></threadedComment><threadedComment ref="A2" dT="2024-01-02T00:00:00Z" id="{44444444-4444-4444-4444-444444444444}" parentId="{11111111-1111-1111-1111-111111111111}" personId="{22222222-2222-2222-2222-222222222222}"><mentions><mention personId="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}" mentionpersonId="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}" mentionId="{66666666-6666-6666-6666-666666666666}" displayName="MENTION_NAME_SECRET"/></mentions><text>REPLY_SECRET_TEXT</text></threadedComment><extLst><ext uri="{D4E1A1F8-D4E1-A1F8-0000-400080000001}"><tcExt count="3"><checksum>1234567890</checksum></tcExt></ext></extLst></threadedComments>"#;
    let persons = r#"<personList xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><person id="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}" displayName="PERSON_SECRET_NAME" userId="USER_SECRET_ID" providerId="PROVIDER_SECRET_ID"/><person id="{22222222-2222-2222-2222-222222222222}" displayName="PERSON_TWO_NAME" userId="USER_TWO_ID" providerId="PROVIDER_TWO_ID"/></personList>"#;
    let comments_out = scrub(
        Format::Xlsx,
        "xl/threadedComments/threadedComment1.xml",
        comments.as_bytes(),
    );
    let persons_out = scrub(Format::Xlsx, "xl/persons/person.xml", persons.as_bytes());
    for secret in [
        "THREAD_SECRET_TEXT",
        "REPLY_SECRET_TEXT",
        "2024-01-01",
        "MENTION_NAME_SECRET",
        "PERSON_SECRET_NAME",
        "PERSON_TWO_NAME",
        "USER_SECRET_ID",
        "USER_TWO_ID",
        "PROVIDER_SECRET_ID",
        "PROVIDER_TWO_ID",
        "{11111111-1111-1111-1111-111111111111}",
        "{44444444-4444-4444-4444-444444444444}",
        "{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}",
        "{22222222-2222-2222-2222-222222222222}",
        "{66666666-6666-6666-6666-666666666666}",
        "1234567890",
    ] {
        assert!(
            !comments_out.contains(secret) && !persons_out.contains(secret),
            "{secret} survived"
        );
    }
    assert!(
        comments_out.contains("<checksum>0</checksum>"),
        "checksum element text must be neutralized: {comments_out}"
    );
    assert!(comments_out.contains("ref=\"A1\""));
    assert_xsd_date_time(&attribute_value(&comments_out, "dT").unwrap());

    let comment_ids = tag_attribute_values(&comments_out, "threadedComment", "id");
    let parent_ids = tag_attribute_values(&comments_out, "threadedComment", "parentId");
    let comment_person_ids = tag_attribute_values(&comments_out, "threadedComment", "personId");
    let mention_ids = tag_attribute_values(&comments_out, "mention", "mentionId");
    let mention_person_ids = tag_attribute_values(&comments_out, "mention", "personId");
    let mention_mention_ids = tag_attribute_values(&comments_out, "mention", "mentionpersonId");
    let person_ids = tag_attribute_values(&persons_out, "person", "id");
    for value in comment_ids
        .iter()
        .chain(parent_ids.iter())
        .chain(comment_person_ids.iter())
        .chain(mention_ids.iter())
        .chain(mention_person_ids.iter())
        .chain(mention_mention_ids.iter())
        .chain(person_ids.iter())
    {
        assert_guid(value);
    }
    assert_eq!(
        parent_ids[0], comment_ids[0],
        "reply parentId must track parent threadedComment id"
    );
    assert_eq!(
        mention_mention_ids[0], comment_person_ids[0],
        "mention@mentionpersonId must track the mentioned threadedComment@personId"
    );
    assert_eq!(
        mention_person_ids[0], person_ids[0],
        "mention@personId must track person@id"
    );
}

#[test]
fn threaded_comment_person_identity_maps_consistently_across_parts() {
    let comments = r#"<threadedComments xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><threadedComment ref="A1" dT="2024-01-01T00:00:00Z" personId="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}"><text>THREAD_SECRET_TEXT</text></threadedComment></threadedComments>"#;
    let persons = r#"<personList xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><person id="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}" displayName="PERSON_SECRET_NAME" userId="USER_SECRET_ID" providerId="PROVIDER_SECRET_ID"/></personList>"#;
    let mut report = RedactionReport::default();
    let mut mappings = IdMappings::default();
    let comments_out = scrub_shared(
        Format::Xlsx,
        "xl/threadedComments/threadedComment1.xml",
        comments.as_bytes(),
        &mut RunState {
            report: &mut report,
            mappings: &mut mappings,
        },
    );
    let persons_out = scrub_shared(
        Format::Xlsx,
        "xl/persons/person.xml",
        persons.as_bytes(),
        &mut RunState {
            report: &mut report,
            mappings: &mut mappings,
        },
    );
    assert!(!comments_out.contains("{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}"));
    let comment_person_id = attribute_value(&comments_out, "personId").unwrap();
    let person_id = attribute_value(&persons_out, "id").unwrap();
    assert_guid(&comment_person_id);
    assert_guid(&person_id);
    assert_eq!(
        comment_person_id, person_id,
        "matching originals must share one placeholder across parts"
    );
}

#[test]
fn database_connection_strings_are_scrubbed() {
    let connections = r#"<connections xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><connection id="1" name="db" type="101" keepAlive="1" odcFile="ODC_FILE_PATH_SECRET" sourceFile="SOURCE_FILE_PATH_SECRET"><dbPr connection="CONNECTION_STRING_SECRET" command="COMMAND_TEXT_SECRET" serverCommand="SERVER_COMMAND_SECRET"/><olapPr sendLocale="1" localConnection="OLAP_CONNECTION_SECRET"/><webPr htmlTables="1" url="WEB_QUERY_URL_SECRET"/></connection><parameters count="5"><parameter name="Region" string="PARAMETER_VALUE_SECRET"/><parameter name="Limit" double="99.5"/><parameter name="Enabled" boolean="1"/><parameter name="Hint" prompt="PROMPT_TEXT_SECRET"/><parameter name="Anchor" cell="CELL_REF_SECRET"/></parameters></connections>"#;
    let output = scrub(Format::Xlsx, "xl/connections.xml", connections.as_bytes());
    for secret in [
        "CONNECTION_STRING_SECRET",
        "COMMAND_TEXT_SECRET",
        "SERVER_COMMAND_SECRET",
        "OLAP_CONNECTION_SECRET",
        "WEB_QUERY_URL_SECRET",
        "PARAMETER_VALUE_SECRET",
        "ODC_FILE_PATH_SECRET",
        "SOURCE_FILE_PATH_SECRET",
        "PROMPT_TEXT_SECRET",
        "CELL_REF_SECRET",
        "99.5",
    ] {
        assert!(!output.contains(secret), "{secret} survived");
    }
    for preserved in ["keepAlive=\"1\"", "sendLocale=\"1\"", "htmlTables=\"1\""] {
        assert!(output.contains(preserved), "{preserved} was rewritten");
    }
    let names = tag_attribute_values(&output, "parameter", "name");
    assert_eq!(names.len(), 5);
    let connection_name = tag_attribute(&output, "connection ", "name").unwrap();
    assert_ref_name(&connection_name);
    for value in &names {
        assert_ref_name(value);
    }
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "distinct originals diverged");
    let strings = tag_attribute_values(&output, "parameter", "string");
    assert_eq!(strings.len(), 1);
    assert!(
        strings[0].bytes().all(|byte| byte == b'x'),
        "{}",
        strings[0]
    );
    for attribute in ["prompt", "cell"] {
        let values = tag_attribute_values(&output, "parameter", attribute);
        assert_eq!(values.len(), 1, "{attribute}");
        assert!(values[0].bytes().all(|byte| byte == b'x'), "{}", values[0]);
    }
    for attribute in ["double", "boolean"] {
        let values = tag_attribute_values(&output, "parameter", attribute);
        assert_eq!(values, vec!["0".to_owned()], "{attribute}");
    }
}

#[test]
fn pivot_cache_refresh_metadata_is_scrubbed() {
    let definition = r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" invalid="1" refreshOnLoad="1" refreshedBy="REFRESHED_BY_SECRET" refreshedDate="45418.297222222226" refreshedDateIso="2024-05-06T07:08:09Z" createdVersion="1" refreshedVersion="1" recordCount="1"><cacheSource type="worksheet"><worksheetSource ref="A1:C4" sheet="Data"/></cacheSource></pivotCacheDefinition>"#;
    let output = scrub(
        Format::Xlsx,
        "xl/pivotCache/pivotCacheDefinition1.xml",
        definition.as_bytes(),
    );
    assert!(!output.contains("REFRESHED_BY_SECRET"));
    assert!(output.contains(r#"refreshedBy="xxxxxxxxxxxxxxxxxxx""#));
    assert!(!output.contains("2024-05-06"));
    assert!(!output.contains("45418.297222222226"));
    assert_eq!(
        attribute_value(&output, "refreshedDate").as_deref(),
        Some("45000")
    );
    assert_eq!(
        attribute_value(&output, "refreshedDateIso").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
    assert!(output.contains(r#"recordCount="1""#));
}

#[test]
fn pivot_cache_definition_names_and_sources_are_scrubbed() {
    let definition = r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" refreshedDate="45418.297222222226"><cacheSource type="worksheet"><worksheetSource ref="A1:C4" sheet="SHEET_NAME_SECRET" name="RANGE_NAME_SECRET"/></cacheSource><cacheFields count="1"><cacheField name="FIELD_NAME_SECRET" caption="FIELD_CAPTION_SECRET" propertyName="PROPERTY_NAME_SECRET" formula="FIELD_FORMULA_SECRET" numFmtId="0"><sharedItems count="1"><s v="PIVOT_CACHE_SECRET"/></sharedItems></cacheField></cacheFields><calculatedItems count="1"><calculatedItem index="0"><formula>CALC_ITEM_FORMULA_SECRET</formula></calculatedItem></calculatedItems></pivotCacheDefinition>"#;
    let slicer_cache = r#"<slicerCacheDefinition xmlns="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" name="CACHE_NAME_OK" sourceName="FIELD_NAME_SECRET"/>"#;
    let output = scrub(
        Format::Xlsx,
        "xl/pivotCache/pivotCacheDefinition1.xml",
        definition.as_bytes(),
    );
    for secret in [
        "45418.297222222226",
        "SHEET_NAME_SECRET",
        "RANGE_NAME_SECRET",
        "FIELD_NAME_SECRET",
        "FIELD_CAPTION_SECRET",
        "PROPERTY_NAME_SECRET",
        "FIELD_FORMULA_SECRET",
        "CALC_ITEM_FORMULA_SECRET",
    ] {
        assert!(!output.contains(secret), "{secret} survived");
    }
    assert_eq!(
        attribute_value(&output, "refreshedDate").as_deref(),
        Some("45000")
    );
    assert!(output.contains("<formula>0</formula>"), "{output}");
    let field_name = tag_attribute(&output, "cacheField ", "name").unwrap();
    let field_caption = tag_attribute(&output, "cacheField ", "caption").unwrap();
    assert_ref_name(&field_name);
    assert_ref_name(&field_caption);
    let property_name = attribute_value(&output, "propertyName").unwrap();
    assert_eq!(
        property_name,
        "x".repeat("PROPERTY_NAME_SECRET".len()),
        "cacheField@propertyName must mask like other pivot labels"
    );
    for (tag, attribute) in [("worksheetSource", "sheet"), ("worksheetSource", "name")] {
        let value = tag_attribute_values(&output, tag, attribute).remove(0);
        assert!(
            !value.is_empty() && value.bytes().all(|byte| byte == b'x'),
            "{attribute}={value} is not a masked name"
        );
    }
    let worksheet_sheet = tag_attribute(&output, "worksheetSource ", "sheet").unwrap();
    let expected_mask = "x".repeat("SHEET_NAME_SECRET".len());
    assert_eq!(
        worksheet_sheet, expected_mask,
        "sheet must mask like sheet@name"
    );
    let cache_out = scrub(
        Format::Xlsx,
        "xl/slicerCaches/slicerCache1.xml",
        slicer_cache.as_bytes(),
    );
    assert_eq!(
        attribute_value(&cache_out, "sourceName").unwrap(),
        field_name,
        "slicer sourceName must track cacheField@name"
    );
}

#[test]
fn pivot_table_item_names_are_scrubbed() {
    let pivot_table = r#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="PivotTable1" dataCaption="DATA_CAPTION_SECRET" rowHeaderCaption="ROW_HEADER_CAPTION_SECRET" cacheId="0"><pivotFields count="1"><pivotField axis="axisRow" showAll="0" name="PIVOT_FIELD_NAME_SECRET" subtotalCaption="SUBTOTAL_CAPTION_SECRET"><items count="2"><item x="0"/><item n="MEMBER_LABEL_SECRET"/></items></pivotField></pivotFields><rowFields count="1"><field x="0"/></rowFields><dataFields count="1"><dataField name="DATA_FIELD_NAME_SECRET" fld="0" baseField="0" baseItem="0"/></dataFields></pivotTableDefinition>"#;
    let slicer_cache = r#"<slicerCacheDefinition xmlns="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" name="CACHE_NAME_SECRET" sourceName="MEMBER_LABEL_SECRET"><pivotTables count="1"><pivotTable tabId="1" name="PivotTable1"/></pivotTables></slicerCacheDefinition>"#;
    let table_out = scrub(
        Format::Xlsx,
        "xl/pivotTables/pivotTable1.xml",
        pivot_table.as_bytes(),
    );
    let cache_out = scrub(
        Format::Xlsx,
        "xl/slicerCaches/slicerCache1.xml",
        slicer_cache.as_bytes(),
    );
    for secret in [
        "MEMBER_LABEL_SECRET",
        "PIVOT_FIELD_NAME_SECRET",
        "SUBTOTAL_CAPTION_SECRET",
        "DATA_CAPTION_SECRET",
        "ROW_HEADER_CAPTION_SECRET",
        "DATA_FIELD_NAME_SECRET",
    ] {
        assert!(!table_out.contains(secret), "{secret} survived");
    }
    let item_names = tag_attribute_values(&table_out, "item", "n");
    assert_eq!(item_names.len(), 1);
    assert_ref_name(&item_names[0]);
    let source_name = attribute_value(&cache_out, "sourceName").unwrap();
    assert_ref_name(&source_name);
    assert_eq!(
        item_names[0], source_name,
        "equal originals must share one RefName placeholder"
    );
    let pivot_field_name = tag_attribute(&table_out, "pivotField ", "name").unwrap();
    let pivot_field_subtotal = tag_attribute(&table_out, "pivotField ", "subtotalCaption").unwrap();
    assert_eq!(
        pivot_field_name,
        "x".repeat("PIVOT_FIELD_NAME_SECRET".len()),
        "pivotField@name must mask like other pivot labels"
    );
    assert_eq!(
        pivot_field_subtotal,
        "x".repeat("SUBTOTAL_CAPTION_SECRET".len()),
        "pivotField@subtotalCaption must mask like other pivot labels"
    );
    for (attribute, secret) in [
        ("dataCaption", "DATA_CAPTION_SECRET"),
        ("rowHeaderCaption", "ROW_HEADER_CAPTION_SECRET"),
    ] {
        let value = attribute_value(&table_out, attribute).unwrap();
        assert_eq!(
            value,
            "x".repeat(secret.len()),
            "{attribute} must mask like other pivot labels"
        );
    }
    let data_field_name = tag_attribute(&table_out, "dataField ", "name").unwrap();
    assert_eq!(
        data_field_name,
        "x".repeat("DATA_FIELD_NAME_SECRET".len()),
        "dataField@name must mask like other pivot labels"
    );
    let definition_name = tag_attribute(&table_out, "pivotTableDefinition ", "name").unwrap();
    let cached_table_name = tag_attribute(&cache_out, "pivotTable ", "name").unwrap();
    assert_ref_name(&definition_name);
    assert_eq!(
        definition_name, cached_table_name,
        "slicerCache pivotTable@name must track pivotTableDefinition@name"
    );
    assert_ne!(
        definition_name,
        attribute_value(&cache_out, "name").unwrap()
    );
}

#[test]
fn chart_pivot_source_compound_names_use_ref_mappings() {
    let chart_xml = |name: &str| {
        format!(
            r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:pivotSource><c:name>{name}</c:name><c:fmtId val="0"/></c:pivotSource><c:chart><c:plotArea/></c:chart></c:chartSpace>"#
        )
    };
    let pivot_table = r#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="CHART_TABLE_SOURCE" cacheId="0"/>"#;
    let mut report = RedactionReport::default();
    let mut mappings = IdMappings::default();
    let mut state = RunState {
        report: &mut report,
        mappings: &mut mappings,
    };
    let bracketed_out = scrub_shared(
        Format::Xlsx,
        "xl/charts/chart1.xml",
        chart_xml("[CHART_BOOK_SOURCE]CHART_SHEET_SOURCE!CHART_TABLE_SOURCE").as_bytes(),
        &mut state,
    );
    let plain_out = scrub_shared(
        Format::Xlsx,
        "xl/charts/chart2.xml",
        chart_xml("CHART_SHEET_SOURCE!CHART_TABLE_SOURCE").as_bytes(),
        &mut state,
    );
    let table_out = scrub_shared(
        Format::Xlsx,
        "xl/pivotTables/pivotTable1.xml",
        pivot_table.as_bytes(),
        &mut state,
    );
    for output in [&bracketed_out, &plain_out] {
        for secret in ["CHART_SHEET_SOURCE", "CHART_TABLE_SOURCE"] {
            assert!(!output.contains(secret), "{secret} survived");
        }
    }
    let definition_name = attribute_value(&table_out, "name").unwrap();
    assert_ref_name(&definition_name);
    let bracketed_mapped = between(&bracketed_out, "<c:name>", "</c:name>").unwrap();
    let book_segment = between(bracketed_mapped, "[", "]").unwrap();
    assert_eq!(
        book_segment, "CHART_BOOK_SOURCE",
        "bracketed workbook filename must stay verbatim"
    );
    let plain_mapped = between(&plain_out, "<c:name>", "</c:name>").unwrap();
    let bracketed_segments = &bracketed_mapped[book_segment.len() + 2..];
    let plain_split = plain_mapped.split_once('!').unwrap();
    for (sheet_segment, table_segment) in [bracketed_segments.split_once('!').unwrap(), plain_split]
    {
        assert_eq!(
            sheet_segment,
            "x".repeat("CHART_SHEET_SOURCE".len()),
            "chart sheet segment must mask like workbook sheet@name"
        );
        assert_ref_name(table_segment);
        assert_ne!(sheet_segment, table_segment);
        assert_eq!(
            table_segment, definition_name,
            "pivotSource table segment must track pivotTableDefinition@name"
        );
    }
}

#[test]
fn pivot_cache_extras_are_scrubbed() {
    let definition = r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cacheSource type="consolidate"><ranges count="1"><rangeSet ref="A1:C4" sheet="RANGESET_SHEET_SECRET" name="RANGESET_RANGE_NAME_SECRET"/></ranges></cacheSource><cacheHierarchies count="1"><cacheHierarchy caption="HIERARCHY_CAPTION_SECRET" uniqueName="[PRODUCTS].[ALL_PRODUCTS_MEMBERS_SECRET]" dimensionUniqueName="[PRODUCTS_DIMENSION_UNIQUE_SECRET]" allCaption="HIERARCHY_ALL_CAPTION_SECRET" allUniqueName="[PRODUCTS].[ALL_UNIQUE_NAME_SECRET]" defaultMemberUniqueName="[PRODUCTS].[DEFAULT_MEMBER_UNIQUE_SECRET]" displayFolder="DISPLAY_FOLDER_SECRET"/></cacheHierarchies><cacheFields count="1"><cacheField name="EXTRA_FIELD_SECRET" numFmtId="0"/></cacheFields></pivotCacheDefinition>"#;
    let pivot_table = r#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="PivotTable9" cacheId="0"><pageFields count="1"><pageField fld="0" hier="-1" name="PAGE_FIELD_NAME_SECRET" cap="PAGE_FIELD_CAPTION_SECRET"/><pageItem name="PivotTable9"/></pageFields></pivotTableDefinition>"#;
    let mut report = RedactionReport::default();
    let mut mappings = IdMappings::default();
    let mut state = RunState {
        report: &mut report,
        mappings: &mut mappings,
    };
    let definition_out = scrub_shared(
        Format::Xlsx,
        "xl/pivotCache/pivotCacheDefinition1.xml",
        definition.as_bytes(),
        &mut state,
    );
    let table_out = scrub_shared(
        Format::Xlsx,
        "xl/pivotTables/pivotTable1.xml",
        pivot_table.as_bytes(),
        &mut state,
    );
    for secret in [
        "RANGESET_SHEET_SECRET",
        "RANGESET_RANGE_NAME_SECRET",
        "HIERARCHY_CAPTION_SECRET",
        "ALL_PRODUCTS_MEMBERS_SECRET",
        "PRODUCTS_DIMENSION_UNIQUE_SECRET",
        "HIERARCHY_ALL_CAPTION_SECRET",
        "ALL_UNIQUE_NAME_SECRET",
        "DEFAULT_MEMBER_UNIQUE_SECRET",
        "DISPLAY_FOLDER_SECRET",
        "EXTRA_FIELD_SECRET",
        "PAGE_FIELD_NAME_SECRET",
        "PAGE_FIELD_CAPTION_SECRET",
    ] {
        assert!(
            !definition_out.contains(secret) && !table_out.contains(secret),
            "{secret} survived"
        );
    }
    let range_sheet = tag_attribute(&definition_out, "rangeSet ", "sheet").unwrap();
    let range_name = tag_attribute_values(&definition_out, "rangeSet", "name").remove(0);
    assert_eq!(
        range_sheet,
        "x".repeat("RANGESET_SHEET_SECRET".len()),
        "rangeSet@sheet must mask like worksheetSource@sheet"
    );
    assert_eq!(
        range_name,
        "x".repeat("RANGESET_RANGE_NAME_SECRET".len()),
        "rangeSet@name must mask like worksheetSource@name"
    );
    let hierarchy_caption = attribute_value(&definition_out, "caption").unwrap();
    assert_eq!(
        hierarchy_caption,
        "x".repeat("HIERARCHY_CAPTION_SECRET".len())
    );
    let unique_name = attribute_value(&definition_out, "uniqueName").unwrap();
    assert_eq!(
        unique_name.len(),
        "[PRODUCTS].[ALL_PRODUCTS_MEMBERS_SECRET]".len()
    );
    assert!(unique_name.bytes().all(|byte| byte == b'x'));
    let dimension_unique_name = attribute_value(&definition_out, "dimensionUniqueName").unwrap();
    assert_eq!(
        dimension_unique_name.len(),
        "[PRODUCTS_DIMENSION_UNIQUE_SECRET]".len()
    );
    assert!(dimension_unique_name.bytes().all(|byte| byte == b'x'));
    for (attribute, secret) in [
        ("allCaption", "HIERARCHY_ALL_CAPTION_SECRET"),
        ("allUniqueName", "[PRODUCTS].[ALL_UNIQUE_NAME_SECRET]"),
        (
            "defaultMemberUniqueName",
            "[PRODUCTS].[DEFAULT_MEMBER_UNIQUE_SECRET]",
        ),
        ("displayFolder", "DISPLAY_FOLDER_SECRET"),
    ] {
        let value = attribute_values(&definition_out, attribute).remove(0);
        assert_eq!(
            value,
            "x".repeat(secret.len()),
            "{attribute} must mask like other hierarchy names"
        );
    }
    let page_field_name = tag_attribute(&table_out, "pageField ", "name").unwrap();
    let page_field_cap = tag_attribute(&table_out, "pageField ", "cap").unwrap();
    assert_eq!(
        page_field_name,
        "x".repeat("PAGE_FIELD_NAME_SECRET".len()),
        "pageField@name must mask like worksheetSource masks"
    );
    assert_eq!(
        page_field_cap,
        "x".repeat("PAGE_FIELD_CAPTION_SECRET".len()),
        "pageField@cap must mask like worksheetSource masks"
    );
    let page_item = tag_attribute(&table_out, "pageItem ", "name").unwrap();
    let definition_name = tag_attribute(&table_out, "pivotTableDefinition ", "name").unwrap();
    assert_ref_name(&page_item);
    assert_eq!(
        page_item, definition_name,
        "pageItem@name must track pivotTableDefinition@name"
    );
}

#[test]
fn pseudonym_placeholders_vary_between_runs() {
    let source = package(vec![
        (
            "[Content_Types].xml",
            xml(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
        ),
        (
            "word/document.xml",
            xml(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p w:rsidR="00AA11BB" w:rsidP="00CC22DD"><w:r><w:t>x</w:t></w:r></w:p><w:sectPr w:rsidR="00AA11BB"/></w:body></w:document>"#,
            ),
        ),
    ]);
    let extract_rsid = |bytes: &[u8]| {
        let parts = ooxml_opc::unzip_parts(bytes).unwrap();
        let document = part_string(&parts, "word/document.xml");
        attribute_values(&document, "rsidR")
    };
    let first_rsids = extract_rsid(&redact(&source, Format::Docx).unwrap());
    let second_rsids = extract_rsid(&redact(&source, Format::Docx).unwrap());
    assert_eq!(first_rsids.len(), 2);
    assert_eq!(
        first_rsids[0], first_rsids[1],
        "within a run, equal originals must stay equal"
    );
    assert_eight_hex(&first_rsids[0]);
    assert_eight_hex(&second_rsids[0]);
    assert_ne!(
        first_rsids[0], second_rsids[0],
        "FNV-derived pseudonyms must be unlinkable across runs"
    );
}

#[test]
fn document_protection_and_merge_sources_are_scrubbed() {
    let settings = r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:documentProtection w:hash="HASH_SECRET" w:salt="SALT_SECRET" w:hashValue="HASH_VALUE_SECRET" w:saltValue="SALT_VALUE_SECRET" w:cryptSpinCount="100000"/><w:writeProtection w:hash="WRITE_HASH_SECRET" w:salt="WRITE_SALT_SECRET" w:hashValue="WRITE_HASH_VALUE_SECRET" w:saltValue="WRITE_SALT_VALUE_SECRET"/><w:mailMerge><w:headerSource r:id="rId2"/><w:dataSource r:id="rId1"/><w:query w:val="QUERY_SECRET"/><w:mailSubject w:val="MAIL_SUBJECT_SECRET"/><w:addressFieldName w:val="ADDRESS_FIELD_SECRET"/><w:odso><w:udl w:val="UDL_SECRET"/><w:table w:val="MERGE_TABLE_SECRET"/></w:odso></w:mailMerge></w:settings>"#;
    let output = scrub(Format::Docx, "word/settings.xml", settings.as_bytes());
    for secret in [
        "HASH_SECRET",
        "SALT_SECRET",
        "HASH_VALUE_SECRET",
        "SALT_VALUE_SECRET",
        "WRITE_HASH_SECRET",
        "WRITE_SALT_SECRET",
        "WRITE_HASH_VALUE_SECRET",
        "WRITE_SALT_VALUE_SECRET",
        "QUERY_SECRET",
        "MAIL_SUBJECT_SECRET",
        "ADDRESS_FIELD_SECRET",
        "UDL_SECRET",
        "MERGE_TABLE_SECRET",
    ] {
        assert!(!output.contains(secret), "{secret} survived");
    }
    assert!(output.contains(r#"w:cryptSpinCount="100000""#));
    assert!(!output.contains("dataSource=\""));
    assert!(output.contains("<w:headerSource r:id=\"rId2\"/><w:dataSource r:id=\"rId1\"/>"));
    let query_val = attribute_value(&output, "val").unwrap();
    assert_eq!(query_val, "xxxxxxxxxxxx");
}

#[test]
fn rsid_attributes_are_scrubbed() {
    let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p w:rsidR="00AA11BB" w:rsidP="00CC22DD" w:rsidRDefault="00AA11BB"><w:r><w:t>hi</w:t></w:r></w:p><w:sectPr w:rsidR="00AA11BB"/></w:body></w:document>"#;
    let output = scrub(Format::Docx, "word/document.xml", document.as_bytes());
    assert!(!output.contains("00AA11BB"));
    assert!(!output.contains("00CC22DD"));
    let rsid_r = attribute_values(&output, "rsidR");
    assert_eq!(rsid_r.len(), 2);
    for value in &rsid_r {
        assert_eight_hex(value);
    }
    assert_eq!(rsid_r[0], rsid_r[1], "equal originals must stay equal");
    let rsid_default = attribute_value(&output, "rsidRDefault").unwrap();
    assert_eight_hex(&rsid_default);
    assert_eq!(rsid_default, rsid_r[0]);
    let rsid_p = attribute_value(&output, "rsidP").unwrap();
    assert_eight_hex(&rsid_p);
    assert_ne!(rsid_p, rsid_r[0]);
}

#[test]
fn revision_timestamps_are_scrubbed() {
    let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:ins w:id="1" w:author="DOCX_SECRET_AUTHOR" w:date="2024-03-04T09:00:00Z" w:dateUtc="2024-03-04T09:00:00Z"><w:r><w:t>x</w:t></w:r></w:ins><w:del w:id="2" w:author="DOCX_DEL_AUTHOR" w:date="2024-03-04T09:30:00Z"><w:r><w:delText>y</w:delText></w:r></w:del><w:cellIns w:id="5" w:author="CELL_AUTHOR_SECRET" w:date="2024-05-05T05:05:05Z"/><w:cellDel w:id="6" w:author="CELL_AUTHOR_SECRET" w:dateUtc="2024-05-05T05:05:05Z"/><w:cellMerge w:id="7" w:author="CELL_AUTHOR_SECRET" w:date="2024-05-05T05:05:05Z"/><w:customXmlInsRangeStart w:id="8" w:author="XML_AUTHOR_SECRET" w:date="2024-07-07T07:07:07Z"/><w:customXmlDelRangeStart w:id="9" w:author="XML_AUTHOR_SECRET" w:dateUtc="2024-07-07T07:07:07Z"/><w:customXmlMoveRangeEnd w:id="10" w:author="XML_AUTHOR_SECRET" w:date="2024-07-07T07:07:07Z"/></w:body></w:document>"#;
    let output = scrub(Format::Docx, "word/document.xml", document.as_bytes());
    assert!(!output.contains("2024-03-04"));
    assert!(!output.contains("2024-05-05"));
    assert!(!output.contains("2024-07-07"));
    for author_secret in [
        "DOCX_SECRET_AUTHOR",
        "DOCX_DEL_AUTHOR",
        "CELL_AUTHOR_SECRET",
        "XML_AUTHOR_SECRET",
    ] {
        assert!(!output.contains(author_secret), "{author_secret} survived");
    }
    for element in [
        "cellIns",
        "cellDel",
        "cellMerge",
        "customXmlInsRangeStart",
        "customXmlDelRangeStart",
        "customXmlMoveRangeEnd",
    ] {
        assert!(output.contains(element), "{element} was dropped");
    }
    let date = attribute_value(&output, "date").unwrap();
    assert_xsd_date_time(&date);
    let date_utc = attribute_value(&output, "dateUtc").unwrap();
    assert_xsd_date_time(&date_utc);
    assert_eq!(date, date_utc);
}

#[test]
fn pptx_section_names_are_scrubbed() {
    let presentation = r#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"><p:sldIdLst><p:sldId id="256"/></p:sldIdLst><p:extLst><p:ext uri="{521415D9-36F7-43E2-AB2F-B90AF26B5E84}"><p14:sectionLst><p14:section name="PROJECT_SECTION_SECRET" id="{33333333-3333-3333-3333-333333333333}"><p14:sldIdLst><p14:sldId id="256"/></p14:sldIdLst></p14:section></p14:sectionLst></p:ext></p:extLst></p:presentation>"#;
    let output = scrub(
        Format::Pptx,
        "ppt/presentation.xml",
        presentation.as_bytes(),
    );
    assert!(!output.contains("PROJECT_SECTION_SECRET"));
    assert!(output.contains("<p14:section name=\"xxxxxxxxxxxxxxxxxxxxxx\""));
}

#[test]
fn slicer_metadata_is_scrubbed() {
    let slicers = r#"<slicers xmlns="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><slicer name="SLICER_CACHE_NAME_SECRET" caption="SOURCE_FIELD_SECRET" cache="SLICER_CACHE_NAME_SECRET" rowHeight="241300"/></slicers>"#;
    let cache = r#"<slicerCacheDefinition xmlns="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" name="SLICER_CACHE_NAME_SECRET" sourceName="SOURCE_FIELD_SECRET"/>"#;
    let slicers_out = scrub(Format::Xlsx, "xl/slicers/slicer1.xml", slicers.as_bytes());
    let cache_out = scrub(
        Format::Xlsx,
        "xl/slicerCaches/slicerCache1.xml",
        cache.as_bytes(),
    );
    for secret in ["SLICER_CACHE_NAME_SECRET", "SOURCE_FIELD_SECRET"] {
        assert!(
            !slicers_out.contains(secret) && !cache_out.contains(secret),
            "{secret} survived"
        );
    }
    assert!(slicers_out.contains("rowHeight=\"241300\""));
    let slicer_name = attribute_value(&slicers_out, "name").unwrap();
    let slicer_cache = attribute_value(&slicers_out, "cache").unwrap();
    let cache_name = attribute_value(&cache_out, "name").unwrap();
    assert_eq!(slicer_name, cache_name);
    assert_eq!(
        slicer_cache, cache_name,
        "slicer@cache must track slicerCacheDefinition@name"
    );
    assert_ref_name(&cache_name);
    let caption = attribute_value(&slicers_out, "caption").unwrap();
    let source_name = attribute_value(&cache_out, "sourceName").unwrap();
    assert_eq!(
        caption, source_name,
        "equal originals must yield equal placeholders"
    );
    assert_ref_name(&caption);
    assert_ne!(cache_name, caption);
}

#[test]
fn drawing_slicer_names_track_the_slicer_view() {
    let drawing = r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:sle="http://schemas.microsoft.com/office/drawing/2010/slicer" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><xdr:twoCellAnchor><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="2" name="SLICER_SHAPE_NAME_SECRET"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:graphic><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2010/slicer"><sle:slicer name="SLICER_CACHE_NAME_SECRET"/></a:graphicData></xdr:graphic></xdr:graphicFrame></xdr:twoCellAnchor><xdr:twoCellAnchor><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="3" name="SECOND_SLICER_SHAPE_SECRET"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:graphic><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2010/slicer"><x14:slicer name="SLICER_CACHE_NAME_SECRET"/></a:graphicData></xdr:graphic></xdr:graphicFrame></xdr:twoCellAnchor></xdr:wsDr>"#;
    let slicers = r#"<slicers xmlns="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><slicer name="SLICER_CACHE_NAME_SECRET" caption="CAPTION_SECRET" cache="SLICER_CACHE_NAME_SECRET" rowHeight="241300"/></slicers>"#;
    let mut report = RedactionReport::default();
    let mut mappings = IdMappings::default();
    let mut state = RunState {
        report: &mut report,
        mappings: &mut mappings,
    };
    let drawing_out = scrub_shared(
        Format::Xlsx,
        "xl/drawings/slicerDrawing1.xml",
        drawing.as_bytes(),
        &mut state,
    );
    let slicers_out = scrub_shared(
        Format::Xlsx,
        "xl/slicers/slicer1.xml",
        slicers.as_bytes(),
        &mut state,
    );
    for secret in [
        "SLICER_CACHE_NAME_SECRET",
        "SLICER_SHAPE_NAME_SECRET",
        "SECOND_SLICER_SHAPE_SECRET",
    ] {
        assert!(
            !drawing_out.contains(secret) && !slicers_out.contains(secret),
            "{secret} survived"
        );
    }
    let view_name = tag_attribute(&slicers_out, "slicer ", "name").unwrap();
    let cache_ref = tag_attribute(&slicers_out, "slicer ", "cache").unwrap();
    assert_eq!(view_name, cache_ref);
    for tag in ["sle:slicer", "x14:slicer"] {
        let drawing_name = tag_attribute(&drawing_out, &format!("{tag} "), "name").unwrap();
        assert_ref_name(&drawing_name);
        assert_eq!(
            drawing_name, view_name,
            "{tag}@name must track the slicer-view name"
        );
    }
}

#[test]
fn legacy_comment_uids_map_to_threaded_comment_ids() {
    let comments = r#"<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><authors><author>tc={CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}</author><author>LEGACY_AUTHOR_SECRET</author></authors><commentList><comment ref="A1" authorId="0" shapeId="0" uid="{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}"><r><t>LEGACY_COMMENT_SECRET</t></r></comment></commentList></comments>"#;
    let threaded = r#"<threadedComments xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><threadedComment ref="A1" id="{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}" dT="2024-05-06T07:08:09Z" personId="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}"><text>THREAD_SECRET_TEXT</text></threadedComment></threadedComments>"#;
    let mut report = RedactionReport::default();
    let mut mappings = IdMappings::default();
    let comments_out = scrub_shared(
        Format::Xlsx,
        "xl/comments1.xml",
        comments.as_bytes(),
        &mut RunState {
            report: &mut report,
            mappings: &mut mappings,
        },
    );
    let threaded_out = scrub_shared(
        Format::Xlsx,
        "xl/threadedComments/threadedComment1.xml",
        threaded.as_bytes(),
        &mut RunState {
            report: &mut report,
            mappings: &mut mappings,
        },
    );
    for secret in [
        "LEGACY_AUTHOR_SECRET",
        "LEGACY_COMMENT_SECRET",
        "{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}",
    ] {
        assert!(
            !comments_out.contains(secret) && !threaded_out.contains(secret),
            "{secret} survived"
        );
    }
    let legacy_uid = tag_attribute_values(&comments_out, "comment", "uid").remove(0);
    let thread_id = tag_attribute_values(&threaded_out, "threadedComment", "id").remove(0);
    assert_guid(&legacy_uid);
    assert_eq!(
        legacy_uid, thread_id,
        "equal original uid and threadedComment id must share one placeholder"
    );
    let author_text = between(&comments_out, "<author>", "</author>").unwrap();
    assert_eq!(
        author_text,
        format!("tc={legacy_uid}"),
        "author text tc={{guid}} must remap the same guid as comment@uid"
    );
    assert!(
        comments_out.contains(&format!(
            "<author>{}</author>",
            "x".repeat("LEGACY_AUTHOR_SECRET".len())
        )),
        "non-threaded authors keep the plain mask: {comments_out}"
    );
    assert!(comments_out.contains("ref=\"A1\""));
}

#[test]
fn word_comment_timestamps_are_scrubbed() {
    let comments = r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="0" w:author="JANE" w:date="2024-08-08T09:10:11Z"><w:p><w:r><t xml:space="preserve">LEGACY_TEXT</t></w:r></w:p></w:comment></w:comments>"#;
    let extended = r#"<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w15:commentEx w15:paraId="11111111" w15:dateUtc="2024-08-08T09:30:11Z" w15:done="0"/></w15:commentsEx>"#;
    let extensible = r#"<w16ce:commentsExtensible xmlns:w16ce="http://schemas.microsoft.com/office/word/2018/wordml/cex"><w16ce:commentExtensible w16ce:durableId="22222222" w16ce:dateUtc="2024-08-08T09:40:11Z"/></w16ce:commentsExtensible>"#;
    let comments_out = scrub(Format::Docx, "word/comments.xml", comments.as_bytes());
    let extended_out = scrub(
        Format::Docx,
        "word/commentsExtended.xml",
        extended.as_bytes(),
    );
    let extensible_out = scrub(
        Format::Docx,
        "word/commentsExtensible.xml",
        extensible.as_bytes(),
    );
    for output in [&comments_out, &extended_out, &extensible_out] {
        assert!(!output.contains("2024-08-08"), "{output}");
    }
    assert_eq!(
        attribute_value(&comments_out, "date").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
    for output in [&extended_out, &extensible_out] {
        assert_eq!(
            attribute_value(output, "dateUtc").as_deref(),
            Some("1970-01-01T00:00:00Z")
        );
    }
    assert!(extensible_out.contains(r#"durableId="22222222""#));
    assert!(extended_out.contains(r#"done="0""#));
}

#[test]
fn relocated_core_properties_are_scrubbed_by_content_type() {
    let source = package(vec![
        (
            "[Content_Types].xml",
            xml(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="corepkg" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Default Extension="apppkg" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/><Default Extension="custpkg" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
        ),
        (
            "xl/workbook.xml",
            xml(
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheets/></workbook>"#,
            ),
        ),
        (
            "meta/core.corepkg",
            xml(
                r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/"><dc:title>RELOCATED_CORE_TITLE_SECRET</dc:title><dc:creator>RELOCATED_CORE_AUTHOR</dc:creator><dcterms:created>2024-05-06T07:08:09Z</dcterms:created><cp:revision>77</cp:revision><dc:identifier>RELOCATED_CORE_IDENTIFIER</dc:identifier><dc:language>RELOCATED_CORE_LANGUAGE</dc:language><cp:version>RELOCATED_CORE_VERSION</cp:version><cp:contentType>RELOCATED_CORE_CONTENT_TYPE</cp:contentType></cp:coreProperties>"#,
            ),
        ),
        (
            "meta/app.apppkg",
            xml(
                r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Company>RELOCATED_APP_COMPANY</Company><AppVersion>16.0301</AppVersion></Properties>"#,
            ),
        ),
        (
            "meta/custom.custpkg",
            xml(
                r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="RELOCATED_CUSTOM_NAME" linkTarget="RELOCATED_LINK_TARGET_SECRET"><vt:lpwstr>RELOCATED_CUSTOM_VALUE</vt:lpwstr></property></Properties>"#,
            ),
        ),
    ]);
    let output = redact(&source, Format::Xlsx).unwrap();
    let parts = ooxml_opc::unzip_parts(&output).unwrap();
    assert_no_secrets(
        &parts,
        &[
            "RELOCATED_CORE_TITLE_SECRET",
            "RELOCATED_CORE_AUTHOR",
            "2024-05-06",
            "77",
            "RELOCATED_CORE_IDENTIFIER",
            "RELOCATED_CORE_LANGUAGE",
            "RELOCATED_CORE_VERSION",
            "RELOCATED_CORE_CONTENT_TYPE",
            "RELOCATED_APP_COMPANY",
            "16.0301",
            "RELOCATED_CUSTOM_NAME",
            "RELOCATED_LINK_TARGET_SECRET",
            "RELOCATED_CUSTOM_VALUE",
        ],
    );
    let core = part_string(&parts, "meta/core.corepkg");
    assert_eq!(
        between(&core, "<cp:revision>", "</cp:revision>"),
        Some("88"),
        "numeric revision must keep its numeric shape"
    );
    let title_text = between(&core, "<dc:title>", "</dc:title>").unwrap();
    assert_eq!(title_text, "x".repeat("RELOCATED_CORE_TITLE_SECRET".len()));
    for (tag, secret) in [
        ("<dc:identifier>", "RELOCATED_CORE_IDENTIFIER"),
        ("<dc:language>", "RELOCATED_CORE_LANGUAGE"),
        ("<cp:version>", "RELOCATED_CORE_VERSION"),
        ("<cp:contentType>", "RELOCATED_CORE_CONTENT_TYPE"),
    ] {
        let closing = tag.replace('<', "</");
        let text = between(&core, tag, &closing).unwrap();
        assert_eq!(text, "x".repeat(secret.len()), "{tag} must be masked");
    }
    let app = part_string(&parts, "meta/app.apppkg");
    let company = between(&app, "<Company>", "</Company>").unwrap();
    assert_eq!(company, "x".repeat("RELOCATED_APP_COMPANY".len()));
    let custom = part_string(&parts, "meta/custom.custpkg");
    let property_name = attribute_value(&custom, "name").unwrap();
    assert_eq!(property_name, "x".repeat("RELOCATED_CUSTOM_NAME".len()));
    let link_target = attribute_value(&custom, "linkTarget").unwrap();
    assert_eq!(
        link_target,
        "x".repeat("RELOCATED_LINK_TARGET_SECRET".len()),
        "custom property linkTarget must be masked"
    );
}

#[test]
fn content_type_overrides_classify_unusual_extensions() {
    let source = package(vec![
        (
            "[Content_Types].xml",
            xml(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/custom/cache.pvt" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
        ),
        (
            "xl/workbook.xml",
            xml(
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheets/></workbook>"#,
            ),
        ),
        (
            "custom/cache.pvt",
            xml(
                r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" refreshedBy="REFRESHED_BY_SECRET"><cacheFields count="1"><cacheField name="FIELD_NAME_SECRET" numFmtId="0"><sharedItems count="1"><s v="PIVOT_CACHE_SECRET"/></sharedItems></cacheField></cacheFields></pivotCacheDefinition>"#,
            ),
        ),
        (
            "custom/data.xyz",
            xml(r#"<unknown xmlns="urn:example">UNCLASSIFIED_PART_SECRET</unknown>"#),
        ),
    ]);
    let output = redact(&source, Format::Xlsx).unwrap();
    let parts = ooxml_opc::unzip_parts(&output).unwrap();
    assert_no_secrets(
        &parts,
        &[
            "PIVOT_CACHE_SECRET",
            "REFRESHED_BY_SECRET",
            "FIELD_NAME_SECRET",
        ],
    );
    let cache = part_string(&parts, "custom/cache.pvt");
    let unclassified = part_string(&parts, "custom/data.xyz");
    assert!(
        unclassified.contains("UNCLASSIFIED_PART_SECRET"),
        "parts without an xml classification must pass through untouched"
    );
    assert!(cache.contains(r#"refreshedBy="xxxxxxxxxxxxxxxxxxx""#));
    assert!(cache.contains(r#"<s v="xxxxxxxxxxxxxxxxxx"/>"#));
}

#[test]
fn content_type_routing_applies_semantic_rules() {
    let source = package(vec![
        (
            "[Content_Types].xml",
            xml(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/blobs/plot.data" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/><Override PartName="/blobs/link.data" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.connections+xml"/><Override PartName="/blobs/folk.data" ContentType="application/vnd.ms-excel.person+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
        ),
        (
            "xl/workbook.xml",
            xml(
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheets/></workbook>"#,
            ),
        ),
        (
            "blobs/plot.data",
            xml(
                r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:pivotSource><c:name>[Book1]CHART_SHEET_SECRET!CHART_TABLE_SECRET</c:name></c:pivotSource><c:chart><c:plotArea><c:ser><c:tx><c:strRef><c:f>CHART_SHEET_SECRET!$A$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>CHART_LABEL_SECRET</c:v></c:pt></c:strCache></c:strRef></c:tx></c:ser></c:plotArea></c:chart></c:chartSpace>"#,
            ),
        ),
        (
            "blobs/link.data",
            xml(
                r#"<connections xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><connection id="1" name="db" type="101" keepAlive="1" odcFile="ODC_FILE_PATH_SECRET"><dbPr connection="CONNECTION_STRING_SECRET"/><webPr htmlTables="1" url="WEB_QUERY_URL_SECRET"/></connection><parameters count="1"><parameter name="Region" string="PARAMETER_VALUE_SECRET"/></parameters></connections>"#,
            ),
        ),
        (
            "blobs/folk.data",
            xml(
                r#"<personList xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><person id="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}" displayName="PERSON_SECRET_NAME" userId="USER_SECRET_ID" providerId="PROVIDER_SECRET_ID"/></personList>"#,
            ),
        ),
    ]);
    let output = redact(&source, Format::Xlsx).unwrap();
    let parts = ooxml_opc::unzip_parts(&output).unwrap();
    assert_no_secrets(
        &parts,
        &[
            "CHART_SHEET_SECRET",
            "CHART_TABLE_SECRET",
            "CHART_LABEL_SECRET",
            "CONNECTION_STRING_SECRET",
            "WEB_QUERY_URL_SECRET",
            "PARAMETER_VALUE_SECRET",
            "ODC_FILE_PATH_SECRET",
            "PERSON_SECRET_NAME",
            "USER_SECRET_ID",
            "PROVIDER_SECRET_ID",
            "{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}",
        ],
    );
    let chart = part_string(&parts, "blobs/plot.data");
    let pivot_source = between(&chart, "<c:name>", "</c:name>").unwrap();
    assert!(
        pivot_source.starts_with("[Book1]"),
        "bracketed workbook filename must stay verbatim: {pivot_source}"
    );
    let (sheet_segment, table_segment) = pivot_source["[Book1]".len()..].split_once('!').unwrap();
    assert_eq!(
        sheet_segment,
        "x".repeat("CHART_SHEET_SECRET".len()),
        "content-type-routed chart must rewrite the sheet segment"
    );
    assert_ref_name(table_segment);
    let cached_label = between(&chart, "<c:v>", "</c:v>").unwrap();
    assert_eq!(
        cached_label,
        "x".repeat("CHART_LABEL_SECRET".len()),
        "content-type-routed chart must scrub cached labels"
    );
    let connections = part_string(&parts, "blobs/link.data");
    let connection_name = tag_attribute(&connections, "connection ", "name").unwrap();
    assert_ref_name(&connection_name);
    let parameter_name = tag_attribute_values(&connections, "parameter", "name").remove(0);
    assert_ref_name(&parameter_name);
    assert!(connections.contains(r#"keepAlive="1""#));
    let person = part_string(&parts, "blobs/folk.data");
    let person_id = tag_attribute(&person, "person ", "id").unwrap();
    assert_guid(&person_id);
    assert_eq!(
        tag_attribute(&person, "person ", "displayName").unwrap(),
        "x".repeat("PERSON_SECRET_NAME".len())
    );
}

#[test]
fn connection_identity_and_web_query_extras_are_scrubbed() {
    let connections = r#"<connections xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><connection id="1" name="db" type="101" keepAlive="1" description="CONNECTION_DESCRIPTION_SECRET" singleSignOnId="SSO_ID_SECRET"><dbPr connection="CONNECTION_STRING_SECRET"/><webPr htmlTables="1" url="WEB_QUERY_URL_SECRET" post="WEB_POST_SECRET" editPage="EDIT_PAGE_SECRET"><tables count="1"><s v="TABLE_NAME_SECRET"/></tables></webPr><textPr prompt="1" codePage="437" sourceFile="TEXT_SOURCE_FILE_SECRET"/></connection></connections>"#;
    let output = scrub(Format::Xlsx, "xl/connections.xml", connections.as_bytes());
    for secret in [
        "CONNECTION_DESCRIPTION_SECRET",
        "SSO_ID_SECRET",
        "WEB_POST_SECRET",
        "EDIT_PAGE_SECRET",
        "WEB_QUERY_URL_SECRET",
        "TABLE_NAME_SECRET",
        "TEXT_SOURCE_FILE_SECRET",
    ] {
        assert!(!output.contains(secret), "{secret} survived");
    }
    for preserved in ["keepAlive=\"1\"", "prompt=\"1\"", "codePage=\"437\""] {
        assert!(output.contains(preserved), "{preserved} was rewritten");
    }
    let connection_name = tag_attribute(&output, "connection ", "name").unwrap();
    assert_ref_name(&connection_name);
}

#[test]
fn tracked_moves_and_rsid_elements_are_scrubbed() {
    let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p w:rsidR="00AA11BB"><w:moveFromRangeStart w:id="3" w:author="MOVE_AUTHOR_SECRET" w:date="2024-04-05T10:00:00Z"/><w:moveFrom w:id="1" w:author="MOVE_AUTHOR_SECRET" w:date="2024-04-05T10:00:00Z"><w:r><w:t>old</w:t></w:r></w:moveFrom><w:moveToRangeStart w:id="4" w:author="MOVE_AUTHOR_SECRET" w:dateUtc="2024-04-05T10:30:00Z"/><w:moveTo w:id="2" w:author="MOVE_AUTHOR_SECRET" w:date="2024-04-05T10:00:00Z"><w:r><w:t>new</w:t></w:r></w:moveTo><w:moveFromRangeEnd w:id="3"/><w:moveToRangeEnd w:id="4"/></w:p><w:sectPr w:rsidR="00AA11BB"/></w:body></w:document>"#;
    let settings = r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:rsids><w:rsidRoot w:val="00AA11BB"/><w:rsid w:val="00CC22DD"/></w:rsids></w:settings>"#;
    let mut report = RedactionReport::default();
    let mut mappings = IdMappings::default();
    let mut state = RunState {
        report: &mut report,
        mappings: &mut mappings,
    };
    let document_out = scrub_shared(
        Format::Docx,
        "word/document.xml",
        document.as_bytes(),
        &mut state,
    );
    let settings_out = scrub_shared(
        Format::Docx,
        "word/settings.xml",
        settings.as_bytes(),
        &mut state,
    );
    assert!(!document_out.contains("2024-04-05"));
    assert!(!document_out.contains("MOVE_AUTHOR_SECRET"));
    for element in [
        "moveFromRangeStart",
        "moveFrom",
        "moveToRangeStart",
        "moveTo",
        "moveFromRangeEnd",
        "moveToRangeEnd",
    ] {
        assert!(document_out.contains(element), "{element} was dropped");
    }
    assert_eq!(
        attribute_value(&document_out, "date").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
    assert_eq!(
        attribute_value(&document_out, "dateUtc").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
    let rsid_root = tag_attribute(&settings_out, "w:rsidRoot ", "val").unwrap();
    let rsid_val = tag_attribute(&settings_out, "w:rsid ", "val").unwrap();
    let rsid_r = attribute_values(&document_out, "rsidR")[0].clone();
    assert_eight_hex(&rsid_root);
    assert_eight_hex(&rsid_val);
    assert_ne!(rsid_root, rsid_val);
    assert_eq!(
        rsid_root, rsid_r,
        "rsidRoot@val must map like rsid attributes across parts"
    );
}

#[test]
fn colliding_originals_get_distinct_placeholders() {
    assert_eq!(super::stable_hash("1aa9"), super::stable_hash("25054"));
    let definition = r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cacheFields count="2"><cacheField name="1aa9" numFmtId="0"/><cacheField name="25054" numFmtId="0"/></cacheFields></pivotCacheDefinition>"#;
    let settings = r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:rsids><w:rsid w:val="1aa9"/><w:rsid w:val="25054"/></w:rsids></w:settings>"#;
    let cache_out = scrub(
        Format::Xlsx,
        "xl/pivotCache/pivotCacheDefinition1.xml",
        definition.as_bytes(),
    );
    let names = attribute_values(&cache_out, "name");
    assert_eq!(names.len(), 2);
    assert_ne!(
        names[0], names[1],
        "FNV-32 collisions must not merge distinct names"
    );
    assert_eq!(names[0], "r0BCDD871");
    for value in &names {
        assert_ref_name(value);
    }
    let settings_out = scrub(Format::Docx, "word/settings.xml", settings.as_bytes());
    let values = attribute_values(&settings_out, "val");
    assert_eq!(values.len(), 2);
    assert_ne!(values[0], values[1]);
    assert_eq!(values[0], "0BCDD871");
    for value in &values {
        assert_eight_hex(value);
    }
}

fn attribute_values(markup: &str, attribute: &str) -> Vec<String> {
    let pattern = format!("{attribute}=\"");
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = markup[cursor..].find(&pattern) {
        let start = cursor + offset + pattern.len();
        let Some(end) = markup[start..].find('"') else {
            break;
        };
        values.push(markup[start..start + end].to_owned());
        cursor = start + end + 1;
    }
    values
}

fn attribute_value(markup: &str, attribute: &str) -> Option<String> {
    attribute_values(markup, attribute).into_iter().next()
}

fn tag_attribute(markup: &str, tag: &str, attribute: &str) -> Option<String> {
    let marker = format!("<{tag}");
    let offset = markup.find(&marker)?;
    let rest = &markup[offset..];
    let end = rest.find('>')?;
    attribute_value(&rest[..end], attribute)
}

fn between<'a>(markup: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let offset = markup.find(start)? + start.len();
    let rest = &markup[offset..];
    let stop = rest.find(end)?;
    Some(&rest[..stop])
}

fn tag_attribute_values(markup: &str, tag: &str, attribute: &str) -> Vec<String> {
    let mut reader = Reader::from_str(markup);
    reader.config_mut().trim_text(false);
    let mut values = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(start) | Event::Empty(start) => {
                if start.name().local_name().as_ref() == tag.as_bytes() {
                    for attr in start.attributes().flatten() {
                        if attr.key.local_name().as_ref() == attribute.as_bytes() {
                            values.push(String::from_utf8_lossy(&attr.value).into_owned());
                        }
                    }
                }
            }
            Event::Eof => return values,
            _ => {}
        }
    }
}

fn assert_eight_hex(value: &str) {
    assert_eq!(value.len(), 8, "{value}");
    assert!(
        value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{value}"
    );
}

fn assert_ref_name(value: &str) {
    assert!(value.starts_with('r'), "{value}");
    assert_eight_hex(&value[1..]);
}

fn assert_guid(value: &str) {
    let body = value
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .unwrap_or_else(|| panic!("guid must be braced: {value}"));
    let groups: Vec<&str> = body.split('-').collect();
    assert_eq!(groups.len(), 5, "{value}");
    for (group, expected) in groups.iter().zip([8usize, 4, 4, 4, 12]) {
        assert_eq!(group.len(), expected, "{value}");
        assert!(
            group
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'A'..=b'F')),
            "{value} must be uppercase hex"
        );
    }
    assert_eq!(groups[2].chars().next(), Some('4'), "{value} not UUIDv4");
    assert!(
        matches!(groups[3].chars().next(), Some('8' | '9' | 'A' | 'B')),
        "{value} has invalid UUID variant"
    );
}

fn assert_xsd_date_time(value: &str) {
    let pattern = "0000-00-00T00:00:00Z";
    assert_eq!(value.len(), pattern.len(), "{value}");
    for (actual, expected) in value.chars().zip(pattern.chars()) {
        if expected == '0' {
            assert!(actual.is_ascii_digit(), "{value}");
        } else {
            assert_eq!(actual, expected, "{value}");
        }
    }
}

fn scrub(format: Format, path: &str, bytes: &[u8]) -> String {
    let mut report = RedactionReport::default();
    let mut mappings = IdMappings::default();
    scrub_shared(
        format,
        path,
        bytes,
        &mut RunState {
            report: &mut report,
            mappings: &mut mappings,
        },
    )
}

fn scrub_shared(format: Format, path: &str, bytes: &[u8], state: &mut RunState<'_>) -> String {
    let output = xml::redact_xml(format, path, None, bytes, state).unwrap();
    String::from_utf8(output).unwrap()
}

#[test]
fn empty_shared_string_cell_does_not_leak_next_value() {
    // Greptile #68: a self-closing <c t="s"/> has no End event, so its cell
    // type must not bleed into the following untyped numeric cell's value.
    let sheet = concat!(
        r#"<?xml version="1.0"?>"#,
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        r#"<sheetData><row r="1">"#,
        r#"<c r="A1" t="s"/>"#,
        r#"<c r="B1"><v>424242</v></c>"#,
        r#"</row></sheetData></worksheet>"#,
    );
    let mut report = RedactionReport::default();
    let mut mappings = IdMappings::default();
    let output = xml::redact_xml(
        Format::Xlsx,
        "xl/worksheets/sheet1.xml",
        None,
        sheet.as_bytes(),
        &mut RunState {
            report: &mut report,
            mappings: &mut mappings,
        },
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(
        !text.contains("424242"),
        "numeric cell value leaked: {text}"
    );
}

#[test]
fn media_placeholder_keeps_each_format() {
    for (format, ext) in [
        (ImageFormat::Gif, "gif"),
        (ImageFormat::Bmp, "bmp"),
        (ImageFormat::Tiff, "tiff"),
    ] {
        let source = placeholder_image(format);
        let mut report = RedactionReport::default();
        let part = format!("word/media/image1.{ext}");
        let output = media::replace_media(&part, &source, &mut report).unwrap();
        assert_ne!(source, output, "{ext} not redacted");
        assert_eq!(
            image::guess_format(&output).unwrap(),
            format,
            "{ext} format changed"
        );
        assert_eq!(
            image_dimensions(&source),
            image_dimensions(&output),
            "{ext} dims changed"
        );
    }
}

#[test]
fn media_rejects_unencodable_formats() {
    let mut report = RedactionReport::default();
    let error = media::replace_media("word/media/image1.emf", b"not an image", &mut report);
    assert!(matches!(error, Err(RedactError::Image { .. })));
}

const INTEGRATION_XLSX_SECRETS: &[&str] = &[
    "PIVOT_SHEET_SECRET",
    "SOURCE_RANGE_NAME_SECRET",
    "REFRESHED_BY_SECRET",
    "45418.297222222226",
    "PIVOT_CACHE_SECRET",
    "CUSTOMER_FIELD_SECRET",
    "FIELD_CAPTION_SECRET",
    "FIELD_FORMULA_SECRET",
    "CALC_ITEM_FORMULA_SECRET",
    "MEMBER_LABEL_SECRET",
    "SLICER_CACHE_NAME_SECRET",
    "CONNECTION_DESCRIPTION_SECRET",
    "SSO_ID_SECRET",
    "ODC_FILE_PATH_SECRET",
    "SOURCE_FILE_PATH_SECRET",
    "CONNECTION_STRING_SECRET",
    "COMMAND_TEXT_SECRET",
    "WEB_QUERY_URL_SECRET",
    "WEB_POST_SECRET",
    "EDIT_PAGE_SECRET",
    "TABLE_NAME_SECRET",
    "TEXT_SOURCE_FILE_SECRET",
    "THREAD_SECRET_TEXT",
    "REPLY_THREAD_SECRET",
    "TCS2_THREAD_SECRET",
    "https://secret.example/tcs2",
    "CACHE_LABEL_SECRET",
    "CATEGORY_CACHE_SECRET",
    "PERSON_SECRET_NAME",
    "PERSON_TWO_SECRET",
    "USER_SECRET_ID",
    "USER_TWO_SECRET",
    "PROVIDER_SECRET_ID",
    "{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}",
    "{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}",
    "{DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD}",
    "{EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE}",
    "{66666666-6666-6666-6666-666666666666}",
    "2024-05-06",
    "PivotTable1",
    "LEGACY_COMMENT_SECRET",
    "SLICER_SHAPE_SECRET",
];

const INTEGRATION_DOCX_SECRETS: &[&str] = &[
    "MOVE_AUTHOR_SECRET",
    "OLD_MOVE_SECRET",
    "NEW_MOVE_SECRET",
    "{BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB}",
    "{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}",
    "{77777777-7777-7777-7777-777777777777}",
    "{88888888-8888-8888-8888-888888888888}",
    "{99999999-9999-9999-9999-999999999999}",
    "WORD_USER_SECRET",
    "WORD_THREAD_SECRET",
    "WORD_REPLY_SECRET",
    "JANE_DOE",
    "WORD_LEGACY_COMMENT_SECRET",
    "00AA11BB",
    "00CC22DD",
    "2024-04-05",
    "2024-06-07",
    "2024-08-08",
];

#[test]
fn package_redaction_is_secret_free_and_referentially_consistent() {
    assert_integration_xlsx();
    assert_integration_docx();
}

fn assert_no_secrets(parts: &[(String, Vec<u8>)], secrets: &[&str]) {
    for (path, bytes) in parts {
        let text = String::from_utf8_lossy(bytes);
        for secret in secrets {
            assert!(!text.contains(secret), "secret {secret} survived in {path}");
        }
    }
}

fn assert_integration_xlsx() {
    let source = package(vec![
        (
            "[Content_Types].xml",
            xml(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="tcsx" ContentType="application/vnd.ms-excel.threadedcomments+xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/pivotCache/pivotCacheDefinition1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"/><Override PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/><Override PartName="/xl/charts/chart1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/><Override PartName="/xl/connections.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.connections+xml"/><Override PartName="/xl/slicerCaches/slicerCache1.xml" ContentType="application/vnd.ms-excel.slicercache+xml"/><Override PartName="/xl/slicers/slicer1.xml" ContentType="application/vnd.ms-excel.slicer+xml"/><Override PartName="/xl/drawings/slicerDrawing1.xml" ContentType="application/vnd.ms-excel.slicerdrawing+xml"/><Override PartName="/xl/threadedComments/threadedComment1.xml" ContentType="application/vnd.ms-excel.threadedcomments+xml"/><Override PartName="/xl/persons/person.xml" ContentType="application/vnd.ms-excel.person+xml"/><Override PartName="/xl/comments1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
        ),
        (
            "xl/workbook.xml",
            xml(
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="PIVOT_SHEET_SECRET" sheetId="1" r:id="rId1"/></sheets><pivotCaches><pivotCache cacheId="0" r:id="rId4"/></pivotCaches></workbook>"#,
            ),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml"/><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/connections" Target="connections.xml"/></Relationships>"#,
            ),
        ),
        (
            "xl/worksheets/sheet1.xml",
            xml(
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData></worksheet>"#,
            ),
        ),
        (
            "xl/pivotCache/pivotCacheDefinition1.xml",
            xml(
                r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" invalid="1" refreshOnLoad="1" refreshedBy="REFRESHED_BY_SECRET" refreshedDate="45418.297222222226" refreshedDateIso="2024-05-06T07:08:09Z" recordCount="1"><cacheSource type="worksheet"><worksheetSource ref="A1:C4" sheet="PIVOT_SHEET_SECRET" name="SOURCE_RANGE_NAME_SECRET"/></cacheSource><cacheFields count="1"><cacheField name="CUSTOMER_FIELD_SECRET" caption="FIELD_CAPTION_SECRET" formula="FIELD_FORMULA_SECRET" numFmtId="0"><sharedItems count="1"><s v="PIVOT_CACHE_SECRET"/></sharedItems></cacheField></cacheFields><calculatedItems count="1"><calculatedItem index="0"><formula>CALC_ITEM_FORMULA_SECRET</formula></calculatedItem></calculatedItems></pivotCacheDefinition>"#,
            ),
        ),
        (
            "xl/pivotTables/pivotTable1.xml",
            xml(
                r#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="PivotTable1" cacheId="0"><pivotFields count="1"><pivotField axis="axisRow" showAll="0"><items count="2"><item x="0"/><item n="MEMBER_LABEL_SECRET"/></items></pivotField></pivotFields><rowFields count="1"><field x="0"/></rowFields></pivotTableDefinition>"#,
            ),
        ),
        (
            "xl/charts/chart1.xml",
            xml(
                r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:pivotSource><c:name>[Book1]PIVOT_SHEET_SECRET!PivotTable1</c:name><c:fmtId val="0"/></c:pivotSource><c:chart><c:plotArea><c:barChart><c:ser><c:tx><c:strRef><c:f>PIVOT_SHEET_SECRET!$A$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>CACHE_LABEL_SECRET</c:v></c:pt></c:strCache></c:strRef></c:tx><c:cat><c:strRef><c:f>PIVOT_SHEET_SECRET!$B$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>CATEGORY_CACHE_SECRET</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:f>PIVOT_SHEET_SECRET!$C$1</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="1"/><c:pt idx="0"><c:v>42.5</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>"#,
            ),
        ),
        (
            "xl/slicerCaches/slicerCache1.xml",
            xml(
                r#"<slicerCacheDefinition xmlns="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" name="SLICER_CACHE_NAME_SECRET" sourceName="CUSTOMER_FIELD_SECRET"><pivotTables count="1"><pivotTable tabId="1" name="PivotTable1"/></pivotTables></slicerCacheDefinition>"#,
            ),
        ),
        (
            "xl/drawings/slicerDrawing1.xml",
            xml(
                r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:sle="http://schemas.microsoft.com/office/drawing/2010/slicer"><xdr:twoCellAnchor><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="2" name="SLICER_SHAPE_SECRET"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:graphic><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2010/slicer"><sle:slicer name="SLICER_CACHE_NAME_SECRET"/></a:graphicData></xdr:graphic></xdr:graphicFrame></xdr:twoCellAnchor></xdr:wsDr>"#,
            ),
        ),
        (
            "xl/slicers/slicer1.xml",
            xml(
                r#"<slicers xmlns="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><slicer name="SLICER_CACHE_NAME_SECRET" caption="CUSTOMER_FIELD_SECRET" cache="SLICER_CACHE_NAME_SECRET" rowHeight="241300"/></slicers>"#,
            ),
        ),
        (
            "xl/connections.xml",
            xml(
                r#"<connections xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><connection id="1" name="db" type="101" keepAlive="1" description="CONNECTION_DESCRIPTION_SECRET" singleSignOnId="SSO_ID_SECRET" odcFile="ODC_FILE_PATH_SECRET" sourceFile="SOURCE_FILE_PATH_SECRET"><dbPr connection="CONNECTION_STRING_SECRET" command="COMMAND_TEXT_SECRET"/><webPr htmlTables="1" url="WEB_QUERY_URL_SECRET" post="WEB_POST_SECRET" editPage="EDIT_PAGE_SECRET"><tables count="1"><s v="TABLE_NAME_SECRET"/></tables></webPr><textPr prompt="1" codePage="437" sourceFile="TEXT_SOURCE_FILE_SECRET"/></connection></connections>"#,
            ),
        ),
        (
            "xl/threadedComments/threadedComment1.xml",
            xml(
                r#"<threadedComments xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><threadedComment ref="A1" id="{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}" dT="2024-05-06T07:08:09Z" personId="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}"><text>THREAD_SECRET_TEXT</text></threadedComment><threadedComment ref="A2" id="{DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD}" parentId="{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}" dT="2024-05-06T07:09:09Z" personId="{EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE}"><mentions><mention personId="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}" mentionpersonId="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}" mentionId="{66666666-6666-6666-6666-666666666666}" displayName="PERSON_SECRET_NAME"/></mentions><text>REPLY_THREAD_SECRET</text></threadedComment></threadedComments>"#,
            ),
        ),
        (
            "xl/threadedComments/threadedComments2.tcsx",
            xml(
                r#"<threadedComments xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><threadedComment ref="D4" id="{88888888-8888-8888-8888-888888888888}" dT="2024-05-06T07:12:13Z" personId="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}"><text>TCS2_THREAD_SECRET</text></threadedComment><extLst><ext uri="{D4E1A1F8-D4E1-A1F8-0000-400080000001}"><tcExt count="3"><checksum>42424242</checksum><hyperlink url="https://secret.example/tcs2"/><junk>TCS2_RAW_SURVIVOR</junk></tcExt></ext></extLst></threadedComments>"#,
            ),
        ),
        (
            "xl/comments1.xml",
            xml(
                r#"<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><authors><author>tc={CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}</author><author>XLSX_SECRET_AUTHOR</author></authors><commentList><comment ref="A1" authorId="0" shapeId="0" uid="{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}"><r><t>LEGACY_COMMENT_SECRET</t></r></comment></commentList></comments>"#,
            ),
        ),
        (
            "xl/persons/person.xml",
            xml(
                r#"<personList xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments"><person id="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}" displayName="PERSON_SECRET_NAME" userId="USER_SECRET_ID" providerId="PROVIDER_SECRET_ID"/><person id="{EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE}" displayName="PERSON_TWO_SECRET" userId="USER_TWO_SECRET" providerId="PROVIDER_TWO_ID"/></personList>"#,
            ),
        ),
    ]);
    let (output, report) = redact_with_report(&source, Format::Auto).unwrap();
    assert_eq!(report.format, Format::Xlsx);
    let parts = ooxml_opc::unzip_parts(&output).unwrap();
    assert_no_secrets(&parts, INTEGRATION_XLSX_SECRETS);

    let slicers = part_string(&parts, "xl/slicers/slicer1.xml");
    let slicer_cache = part_string(&parts, "xl/slicerCaches/slicerCache1.xml");
    let slicer_drawing = part_string(&parts, "xl/drawings/slicerDrawing1.xml");
    let cache_definition = part_string(&parts, "xl/pivotCache/pivotCacheDefinition1.xml");
    let pivot_table = part_string(&parts, "xl/pivotTables/pivotTable1.xml");
    let workbook = part_string(&parts, "xl/workbook.xml");
    let comments = part_string(&parts, "xl/threadedComments/threadedComment1.xml");
    let legacy_comments = part_string(&parts, "xl/comments1.xml");
    let persons = part_string(&parts, "xl/persons/person.xml");

    let slicer_name = attribute_value(&slicers, "name").unwrap();
    let slicer_cache_ref = attribute_value(&slicers, "cache").unwrap();
    let cache_name = attribute_value(&slicer_cache, "name").unwrap();
    assert_eq!(slicer_name, slicer_cache_ref);
    assert_eq!(
        slicer_cache_ref, cache_name,
        "slicer@cache must track slicerCacheDefinition@name"
    );
    let drawing_name = tag_attribute(&slicer_drawing, "sle:slicer ", "name").unwrap();
    assert_ref_name(&drawing_name);
    assert_eq!(
        drawing_name, slicer_name,
        "drawing slicer@name must track the slicer-view name"
    );
    let caption = attribute_value(&slicers, "caption").unwrap();
    let source_name = attribute_value(&slicer_cache, "sourceName").unwrap();
    let field_name = tag_attribute(&cache_definition, "cacheField ", "name").unwrap();
    assert_eq!(caption, source_name);
    assert_eq!(
        source_name, field_name,
        "slicer sourceName must track cacheField@name"
    );

    let definition_name = tag_attribute(&pivot_table, "pivotTableDefinition ", "name").unwrap();
    let cached_table_name = tag_attribute(&slicer_cache, "pivotTable ", "name").unwrap();
    assert_ref_name(&definition_name);
    assert_eq!(
        definition_name, cached_table_name,
        "slicerCache pivotTable@name must track pivotTableDefinition@name"
    );

    let chart = part_string(&parts, "xl/charts/chart1.xml");
    let pivot_source = between(&chart, "<c:name>", "</c:name>").unwrap();
    let book_segment = between(pivot_source, "[", "]").unwrap();
    assert_eq!(
        book_segment, "Book1",
        "bracketed workbook filename must stay verbatim"
    );
    let sheet_name = tag_attribute(&workbook, "sheet ", "name").unwrap();
    let (chart_sheet_segment, chart_table_segment) = pivot_source[book_segment.len() + 2..]
        .split_once('!')
        .unwrap();
    assert_eq!(
        chart_sheet_segment, sheet_name,
        "chart sheet segment must track the redacted workbook sheet@name"
    );
    assert_ref_name(chart_table_segment);
    assert_ne!(chart_sheet_segment, chart_table_segment);
    assert_eq!(
        chart_table_segment, definition_name,
        "chart pivotSource table segment must track pivotTableDefinition@name"
    );
    assert!(!chart.contains("CACHE_LABEL_SECRET") && !chart.contains("$A$1"));

    let legacy_uid = tag_attribute_values(&legacy_comments, "comment", "uid").remove(0);
    let thread_ids = tag_attribute_values(&comments, "threadedComment", "id");
    assert_guid(&legacy_uid);
    assert_eq!(
        legacy_uid, thread_ids[0],
        "legacy comment@uid must track the threadedComment id it links to"
    );
    let author_text = between(&legacy_comments, "<author>", "</author>").unwrap();
    assert_eq!(
        author_text,
        format!("tc={legacy_uid}"),
        "xl/comments author text tc={{guid}} must reuse the comment@uid mapping"
    );

    let comment_person_id = attribute_value(&comments, "personId").unwrap();
    let person_id = attribute_value(&persons, "id").unwrap();
    assert_guid(&comment_person_id);
    assert_guid(&person_id);
    assert_eq!(
        comment_person_id, person_id,
        "personId must stay consistent across persons and threadedComments"
    );
    let parent_ids = tag_attribute_values(&comments, "threadedComment", "parentId");
    let comment_person_ids = tag_attribute_values(&comments, "threadedComment", "personId");
    let mention_ids = tag_attribute_values(&comments, "mention", "mentionId");
    let mention_mention_ids = tag_attribute_values(&comments, "mention", "mentionpersonId");
    let person_ids = tag_attribute_values(&persons, "person", "id");
    for value in thread_ids
        .iter()
        .chain(parent_ids.iter())
        .chain(comment_person_ids.iter())
        .chain(mention_ids.iter())
        .chain(mention_mention_ids.iter())
        .chain(person_ids.iter())
    {
        assert_guid(value);
    }
    assert_eq!(
        parent_ids[0], thread_ids[0],
        "reply parentId must track parent threadedComment id"
    );
    assert_eq!(
        mention_mention_ids[0], comment_person_ids[0],
        "mention@mentionpersonId must track the mentioned threadedComment@personId"
    );

    let tcs2 = part_string(&parts, "xl/threadedComments/threadedComments2.tcsx");
    assert!(
        !tcs2.contains("TCS2_THREAD_SECRET"),
        "threadedcomment body text must be scrubbed"
    );
    assert!(
        tcs2.contains("<checksum>0</checksum>"),
        "checksum element text must be neutralized: {tcs2}"
    );
    assert!(tcs2.contains(r#"count="3""#));
    assert_eq!(
        attribute_value(&tcs2, "dT").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
    let tcs2_url = tag_attribute(&tcs2, "hyperlink ", "url").unwrap();
    assert_eq!(
        tcs2_url,
        "x".repeat("https://secret.example/tcs2".len()),
        "hyperlink@url must be masked"
    );
    assert_eq!(tag_attribute_values(&tcs2, "hyperlink", "url").len(), 1);
    assert!(
        tcs2.contains("<junk>TCS2_RAW_SURVIVOR</junk>"),
        "text outside comment bodies must pass through untouched: {tcs2}"
    );

    let worksheet_sheet = tag_attribute(&cache_definition, "worksheetSource ", "sheet").unwrap();
    assert_eq!(
        worksheet_sheet, sheet_name,
        "worksheetSource@sheet must equal the redacted workbook sheet name"
    );
    let worksheet_source_name =
        tag_attribute_values(&cache_definition, "worksheetSource", "name").remove(0);
    assert!(
        !worksheet_source_name.is_empty() && worksheet_source_name.bytes().all(|byte| byte == b'x'),
        "worksheetSource@name is not a masked name: {worksheet_source_name}"
    );
    assert_eq!(
        attribute_value(&cache_definition, "refreshedDate").as_deref(),
        Some("45000")
    );
    assert_eq!(
        attribute_value(&cache_definition, "refreshedDateIso").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
    assert!(cache_definition.contains("<formula>0</formula>"));
}

fn assert_integration_docx() {
    let source = package(vec![
        (
            "[Content_Types].xml",
            xml(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/><Override PartName="/word/commentsExtended.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml"/><Override PartName="/word/commentsExtensible.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtensible+xml"/><Override PartName="/word/people.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.people+xml"/><Override PartName="/word/threadedComments/threadedComment1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.threadedComments+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            xml(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
        ),
        (
            "word/document.xml",
            xml(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p w:rsidR="00AA11BB" w:rsidRDefault="00AA11BB"><w:moveFromRangeStart w:id="3" w:author="MOVE_AUTHOR_SECRET" w:date="2024-04-05T10:00:00Z"/><w:moveFrom w:id="1" w:author="MOVE_AUTHOR_SECRET" w:date="2024-04-05T10:00:00Z"><w:r><w:t>OLD_MOVE_SECRET</w:t></w:r></w:moveFrom><w:moveTo w:id="2" w:author="MOVE_AUTHOR_SECRET" w:date="2024-04-05T10:00:00Z"><w:r><w:t>NEW_MOVE_SECRET</w:t></w:r></w:moveTo><w:moveFromRangeEnd w:id="3"/><w:moveToRangeStart w:id="4" w:author="MOVE_AUTHOR_SECRET" w:dateUtc="2024-04-05T10:30:00Z"/><w:moveToRangeEnd w:id="4"/></w:p><w:sectPr w:rsidR="00AA11BB"/></w:body></w:document>"#,
            ),
        ),
        (
            "word/settings.xml",
            xml(
                r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:zoom w:percent="100"/><w:rsids><w:rsidRoot w:val="00AA11BB"/><w:rsid w:val="00CC22DD"/></w:rsids></w:settings>"#,
            ),
        ),
        (
            "word/people.xml",
            xml(
                r#"<w15:persons xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml/person"><w15:person w15:author="JANE_DOE"><w15:presenceInfo w15:userId="WORD_USER_SECRET" w15:providerId="None"/></w15:person></w15:persons>"#,
            ),
        ),
        (
            "word/comments.xml",
            xml(
                r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w:comment w:id="0" w:author="JANE_DOE" w:date="2024-08-08T09:10:11Z" w15:paraId="11111111" w15:textId="22222222"><w:p><w:r><w:t>WORD_LEGACY_COMMENT_SECRET</w:t></w:r></w:p></w:comment></w:comments>"#,
            ),
        ),
        (
            "word/commentsExtended.xml",
            xml(
                r#"<w15:commentsEx xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w15:commentEx w15:paraId="11111111" w15:dateUtc="2024-08-08T09:30:11Z" w15:done="0"/></w15:commentsEx>"#,
            ),
        ),
        (
            "word/commentsExtensible.xml",
            xml(
                r#"<w16ce:commentsExtensible xmlns:w16ce="http://schemas.microsoft.com/office/word/2018/wordml/cex"><w16ce:commentExtensible w16ce:durableId="33333333" w16ce:dateUtc="2024-08-08T09:40:11Z"/></w16ce:commentsExtensible>"#,
            ),
        ),
        (
            "word/threadedComments/threadedComment1.xml",
            xml(
                r#"<wtp:threadedComments xmlns:wtp="http://schemas.microsoft.com/office/word/2018/wordprocessing/threadedComments"><wtp:threadedComment wtp:id="{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}" wtp:date="2024-06-07T11:12:13Z" wtp:personId="{BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB}"><wtp:text>WORD_THREAD_SECRET</wtp:text></wtp:threadedComment><wtp:threadedComment wtp:id="{77777777-7777-7777-7777-777777777777}" wtp:parentId="{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}" wtp:date="2024-06-07T11:14:15Z" wtp:personId="{88888888-8888-8888-8888-888888888888}"><wtp:mentions><wtp:mention wtp:personId="{BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB}" wtp:mentionpersonId="{BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB}" wtp:mentionId="{99999999-9999-9999-9999-999999999999}" wtp:userId="WORD_USER_SECRET"/></wtp:mentions><wtp:text>WORD_REPLY_SECRET</wtp:text></wtp:threadedComment></wtp:threadedComments>"#,
            ),
        ),
    ]);
    let (output, report) = redact_with_report(&source, Format::Docx).unwrap();
    assert_eq!(report.format, Format::Docx);
    let parts = ooxml_opc::unzip_parts(&output).unwrap();
    assert_no_secrets(&parts, INTEGRATION_DOCX_SECRETS);

    let document = part_string(&parts, "word/document.xml");
    let settings = part_string(&parts, "word/settings.xml");
    let people = part_string(&parts, "word/people.xml");
    let legacy_comments = part_string(&parts, "word/comments.xml");
    let comments_extended = part_string(&parts, "word/commentsExtended.xml");
    let comments_extensible = part_string(&parts, "word/commentsExtensible.xml");
    let comments = part_string(&parts, "word/threadedComments/threadedComment1.xml");

    let person_author = tag_attribute(&people, "w15:person ", "author").unwrap();
    let comment_author = tag_attribute(&legacy_comments, "w:comment ", "author").unwrap();
    assert_eq!(
        person_author, comment_author,
        "person@author must mask exactly like annotation authors"
    );
    assert_eq!(person_author, "xxxxxxxx", "authors must be masked strings");

    for output in [
        &legacy_comments,
        &comments_extended,
        &comments_extensible,
        &comments,
    ] {
        assert!(!output.contains("2024-08-08") && !output.contains("2024-06-07"));
    }
    assert_eq!(
        attribute_value(&legacy_comments, "date").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
    assert_eq!(
        attribute_value(&comments_extended, "dateUtc").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
    assert_eq!(
        attribute_value(&comments_extensible, "dateUtc").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );

    let comment_person_id = attribute_value(&comments, "personId").unwrap();
    assert_guid(&comment_person_id);
    let comment_ids = tag_attribute_values(&comments, "threadedComment", "id");
    let parent_ids = tag_attribute_values(&comments, "threadedComment", "parentId");
    let mention_ids = tag_attribute_values(&comments, "mention", "mentionId");
    let mention_mention_ids = tag_attribute_values(&comments, "mention", "mentionpersonId");
    for value in comment_ids
        .iter()
        .chain(parent_ids.iter())
        .chain(mention_ids.iter())
        .chain(mention_mention_ids.iter())
    {
        assert_guid(value);
    }
    assert_eq!(
        parent_ids[0], comment_ids[0],
        "reply parentId must track parent threadedComment id"
    );
    assert_eq!(
        mention_mention_ids[0], comment_person_id,
        "mention@mentionpersonId must track the mentioned threadedComment@personId"
    );

    let rsid_root = tag_attribute(&settings, "w:rsidRoot ", "val").unwrap();
    let rsid_r = attribute_values(&document, "rsidR")[0].clone();
    assert_eight_hex(&rsid_root);
    assert_eq!(
        rsid_root, rsid_r,
        "rsidRoot@val must map like rsid attributes across parts"
    );

    assert_eq!(
        attribute_value(&document, "date").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
    assert_eq!(
        attribute_value(&comments, "date").as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
}

fn part_string(parts: &[(String, Vec<u8>)], path: &str) -> String {
    String::from_utf8(part(parts, path).to_vec()).unwrap()
}
