use std::sync::{Arc, Mutex};

use pptx_edit::{CommentFlavor, DeckSession, DeckSnapshot, EditCtx, EditError, TextStyle};

const DEMO: &[u8] = include_bytes!("../../../apps/demo/public/betteroffice-demo.pptx");
const MODERN: &[u8] = include_bytes!("fixtures/modern-comments.pptx");

#[test]
fn comment_operations_are_individually_undoable_and_publish_one_update() {
    let session = DeckSession::open(DEMO, 711).unwrap();
    let context = EditCtx::local("test");
    let initial = session.snapshot().unwrap();
    let slide = &initial.slides[0];
    let story = &slide
        .shapes
        .iter()
        .find(|shape| !shape.text_stories.is_empty())
        .unwrap()
        .text_stories[0]
        .id;
    let updates = Arc::new(Mutex::new(Vec::new()));
    let recorded = updates.clone();
    let _subscription = session
        .observe_update_v1(move |update| recorded.lock().unwrap().push(update))
        .unwrap();
    let mut states = vec![initial.clone()];
    session
        .insert_text(&context, story, 0, "Before", &TextStyle::default())
        .unwrap();
    states.push(session.snapshot().unwrap());
    session
        .set_comment_flavor(&context, CommentFlavor::Modern)
        .unwrap();
    states.push(session.snapshot().unwrap());
    let root = session
        .add_comment(
            &context,
            &slide.id,
            "Ada",
            "AL",
            "Root 😀",
            "2026-09-05T10:00:00Z",
            12345,
            -6789,
        )
        .unwrap();
    states.push(session.snapshot().unwrap());
    session
        .reply_to_comment(
            &context,
            &root.comment_id,
            "Grace",
            "GH",
            "Reply",
            "2026-09-05T10:01:00Z",
        )
        .unwrap();
    states.push(session.snapshot().unwrap());
    session
        .set_comment_status(&context, &root.comment_id, true)
        .unwrap();
    states.push(session.snapshot().unwrap());
    session.remove_comment(&context, &root.comment_id).unwrap();
    states.push(session.snapshot().unwrap());
    session
        .insert_text(&context, story, 0, "After", &TextStyle::default())
        .unwrap();
    states.push(session.snapshot().unwrap());
    assert_eq!(updates.lock().unwrap().len(), 7);
    let peer = DeckSession::open(DEMO, 712).unwrap();
    for (index, event) in updates.lock().unwrap().iter().enumerate() {
        peer.apply_update_v1(&event.update).unwrap();
        assert_eq!(peer.snapshot().unwrap(), states[index + 1]);
    }
    for expected in states[..states.len() - 1].iter().rev() {
        assert!(session.undo());
        assert_eq!(&session.snapshot().unwrap(), expected);
    }
    assert!(!session.can_undo());
    for expected in &states[1..] {
        assert!(session.redo());
        assert_eq!(&session.snapshot().unwrap(), expected);
    }
    assert!(!session.can_redo());
}

#[test]
fn comment_strings_reject_xml_controls_without_committing() {
    let session = DeckSession::open(DEMO, 713).unwrap();
    let context = EditCtx::local("test");
    session
        .set_comment_flavor(&context, CommentFlavor::Modern)
        .unwrap();
    let slide = session.snapshot().unwrap().slides[0].id.clone();
    let root = session
        .add_comment(
            &context,
            &slide,
            "Ada",
            "AL",
            "Root",
            "2026-09-05T10:00:00Z",
            0,
            0,
        )
        .unwrap();
    let before = session.encode_state_as_update_v1();
    for index in 0..4 {
        let mut fields = ["Ada", "AL", "Text", "2026-09-05T10:00:00Z"];
        fields[index] = "bad\0value";
        assert!(matches!(
            session.add_comment(
                &context, &slide, fields[0], fields[1], fields[2], fields[3], 0, 0
            ),
            Err(EditError::InvalidText(_))
        ));
        assert!(matches!(
            session.reply_to_comment(
                &context,
                &root.comment_id,
                fields[0],
                fields[1],
                fields[2],
                fields[3]
            ),
            Err(EditError::InvalidText(_))
        ));
        assert_eq!(session.encode_state_as_update_v1(), before);
    }
}

#[test]
fn modern_positions_and_plain_text_follow_drawingml() {
    let session = DeckSession::open(MODERN, 714).unwrap();
    let comments = session.comments().unwrap();
    let root = comments
        .iter()
        .find(|comment| comment.created.as_deref() == Some("2024-12-30T20:26:06.503Z"))
        .unwrap();
    assert_eq!((root.x_emu, root.y_emu), (12345, -6789));
    assert_eq!(root.author, "Mary Smith");
    assert_eq!(root.initials, "MS");
    assert_eq!(root.text, "Needs a source.\nEmoji 😀\nSecond paragraph.");
    let slide = session.snapshot().unwrap().slides[2].id.clone();
    session
        .add_comment(
            &EditCtx::local("test"),
            &slide,
            "Ada",
            "AL",
            "First 😀\nSecond",
            "2026-09-05T10:00:00Z",
            12345,
            -6789,
        )
        .unwrap();
    let saved = session.save().unwrap();
    let parts = ooxml_opc::unzip_parts(&saved).unwrap();
    let (_, bytes) = parts
        .iter()
        .find(|(name, _)| name == "ppt/comments/modernComment3.xml")
        .unwrap();
    let xml = String::from_utf8(bytes.clone()).unwrap();
    assert!(xml.contains("<p188:pos x=\"12345\" y=\"-6789\"/>"));
    assert!(!xml.contains("cId=\"0\""));
    assert_eq!(xml.matches("<a:p>").count(), 2);
    let reopened = DeckSession::open(&saved, 715).unwrap();
    let added = reopened
        .comments()
        .unwrap()
        .into_iter()
        .find(|comment| comment.author == "Ada")
        .unwrap();
    assert_eq!((added.x_emu, added.y_emu), (12345, -6789));
    assert_eq!(added.text, "First 😀\nSecond");
}

#[test]
fn default_comment_fields_preserve_snapshot_json() {
    let session = DeckSession::open(DEMO, 716).unwrap();
    let snapshot = session.snapshot().unwrap();
    let json = serde_json::to_value(&snapshot).unwrap();
    assert!(json.get("comments").is_none());
    assert!(json.get("commentFlavor").is_none());
    assert_eq!(
        serde_json::from_value::<DeckSnapshot>(json).unwrap(),
        snapshot
    );
    let package = serde_json::to_value(session.package()).unwrap();
    for key in ["comments", "commentAuthors", "commentFlavor"] {
        assert!(package.get(key).is_none());
    }
    let restored: pptx_parse::PptxPackage = serde_json::from_value(package).unwrap();
    assert!(restored.comments.is_empty());
    assert!(restored.comment_authors.is_empty());
    assert_eq!(restored.comment_flavor, None);
}

#[test]
fn renamed_author_parts_are_reused_and_removed() {
    let mut original = ooxml_opc::unzip_parts(MODERN).unwrap();
    for (path, bytes) in &mut original {
        if path == "ppt/authors.xml" {
            *path = "ppt/people/team.xml".to_owned();
        } else if path == "[Content_Types].xml" || path == "ppt/_rels/presentation.xml.rels" {
            *bytes = String::from_utf8(bytes.clone())
                .unwrap()
                .replace("authors.xml", "people/team.xml")
                .into_bytes();
        }
    }
    let source = ooxml_opc::rezip_parts(&original).unwrap();
    let session = DeckSession::open(&source, 717).unwrap();
    let context = EditCtx::local("test");
    let root = session
        .comments()
        .unwrap()
        .into_iter()
        .find(|comment| comment.parent_id.is_none())
        .unwrap();
    session
        .set_comment_status(&context, &root.id, true)
        .unwrap();
    let saved = ooxml_opc::unzip_parts(&session.save().unwrap()).unwrap();
    assert!(saved.iter().any(|(path, _)| path == "ppt/people/team.xml"));
    assert!(!saved.iter().any(|(path, _)| path == "ppt/authors.xml"));
    let rels = &saved
        .iter()
        .find(|(path, _)| path == "ppt/_rels/presentation.xml.rels")
        .unwrap()
        .1;
    assert!(
        String::from_utf8(rels.clone())
            .unwrap()
            .contains("people/team.xml")
    );
    for root in session
        .comments()
        .unwrap()
        .iter()
        .filter(|comment| comment.parent_id.is_none())
    {
        session.remove_comment(&context, &root.id).unwrap();
    }
    let saved = ooxml_opc::unzip_parts(&session.save().unwrap()).unwrap();
    assert!(
        !saved
            .iter()
            .any(|(path, _)| path == "ppt/people/team.xml" || path.starts_with("ppt/comments/"))
    );
    for (path, bytes) in saved {
        if path == "[Content_Types].xml" || path.ends_with(".rels") {
            let xml = String::from_utf8(bytes).unwrap();
            assert!(!xml.contains("people/team.xml"));
            assert!(!xml.contains("/relationships/comments"));
            assert!(!xml.contains("/relationships/authors"));
        }
    }
}

#[test]
fn deleting_slides_removes_comments_and_last_authors() {
    let session = DeckSession::open(MODERN, 718).unwrap();
    let context = EditCtx::local("test");
    let initial = session.snapshot().unwrap();
    session
        .delete_slide(&context, &initial.slides[0].id)
        .unwrap();
    let comments = session.comments().unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].slide_id, initial.slides[1].id);
    let remaining = "ppt/comments/modernComment_257_3ADE68B1.xml";
    let saved = ooxml_opc::unzip_parts(&session.save().unwrap()).unwrap();
    assert_eq!(
        saved.iter().find(|(path, _)| path == remaining).unwrap().1,
        session.package().part_bytes(remaining).unwrap()
    );
    assert!(session.undo());
    assert_eq!(session.snapshot().unwrap(), initial);
    assert_eq!(
        ooxml_opc::unzip_parts(&session.save().unwrap()).unwrap(),
        ooxml_opc::unzip_parts(MODERN).unwrap()
    );
    assert!(session.redo());
    session
        .delete_slide(&context, &initial.slides[1].id)
        .unwrap();
    assert!(session.comments().unwrap().is_empty());
    let saved = ooxml_opc::unzip_parts(&session.save().unwrap()).unwrap();
    assert!(
        !saved
            .iter()
            .any(|(path, _)| path == "ppt/authors.xml" || path.starts_with("ppt/comments/"))
    );
}
