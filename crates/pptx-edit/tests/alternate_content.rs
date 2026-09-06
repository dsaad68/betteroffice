use pptx_edit::{DeckSession, EditCtx};

const FIXTURE: &[u8] = include_bytes!("../../pptx-render/tests/fixtures/alternate-content.pptx");

fn slide_xml(bytes: &[u8]) -> String {
    let parts = ooxml_opc::unzip_parts(bytes).unwrap();
    let part = parts
        .iter()
        .find(|(path, _)| path == "ppt/slides/slide1.xml")
        .unwrap();
    String::from_utf8(part.1.clone()).unwrap()
}

#[test]
fn a_fill_edit_lands_on_the_shape_the_fallback_holds() {
    let session = DeckSession::open(FIXTURE, 332).unwrap();
    let context = EditCtx::local("test");
    let snapshot = session.snapshot().unwrap();
    let slide = &snapshot.slides[0];
    let names: Vec<&str> = slide
        .shapes
        .iter()
        .map(|shape| shape.name.as_str())
        .collect();
    assert_eq!(names, ["title", "control", "fallback", "caption"]);

    session
        .set_shape_fill(&context, &slide.id, &slide.shapes[2].id, Some("#DC2626"))
        .unwrap();
    session
        .set_shape_fill(&context, &slide.id, &slide.shapes[3].id, Some("#12B76A"))
        .unwrap();
    let xml = slide_xml(&session.save().unwrap());

    let fallback = xml.split("<mc:Fallback>").nth(1).unwrap();
    assert!(fallback.contains(r#"name="fallback""#));
    assert!(fallback.contains(r#"<a:srgbClr val="DC2626"/>"#));
    assert!(xml.contains(r#"<mc:Choice Requires="bo""#));
    let caption = xml.split(r#"name="caption""#).nth(1).unwrap();
    assert!(caption.contains(r#"<a:srgbClr val="12B76A"/>"#));
}

#[test]
fn a_save_without_edits_leaves_the_alternate_content_untouched() {
    let session = DeckSession::open(FIXTURE, 333).unwrap();
    assert_eq!(slide_xml(&session.save().unwrap()), slide_xml(FIXTURE));
}
