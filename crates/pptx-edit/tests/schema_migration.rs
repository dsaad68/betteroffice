//! `deck-schema-v1.update.bin` was produced by release 4bdccdd: it opens
//! `betteroffice-demo.pptx`, adds a text box and edits its story, then persists
//! `encode_state_as_update_v1()`.

use pptx_edit::{DeckSession, DeckSnapshot, EditCtx, EditError, TextStyle};
use yrs::updates::decoder::Decode;
use yrs::{Any, Doc, Map, MapRef, Out, ReadTxn, StateVector, Transact, Update};

const V2_STYLE_UPDATE: &[u8] = include_bytes!("fixtures/deck-schema-v2.update.bin");
const V1_UPDATE: &[u8] = include_bytes!("fixtures/deck-schema-v1.update.bin");
const V2_SOURCE: &[u8] = include_bytes!("fixtures/deck-schema-v2-connectors.pptx");
const V2_UPDATE: &[u8] = include_bytes!("fixtures/deck-schema-v2-connectors.update.bin");
const V2_MOVED_UPDATE: &[u8] =
    include_bytes!("fixtures/deck-schema-v2-connectors-moved.update.bin");
const FIXTURE: &[u8] = include_bytes!("../../../apps/demo/public/betteroffice-demo.pptx");
const NUMBERED_FIXTURE: &[u8] =
    include_bytes!("../../pptx-parse/tests/fixtures/slide-number-fields.pptx");
const V3_CONNECTORS_UPDATE: &[u8] = include_bytes!("fixtures/deck-schema-v3-connectors.update.bin");
const V3_NUMBERED_UPDATE: &[u8] =
    include_bytes!("fixtures/deck-schema-v3-slide-number-fields.update.bin");
const V3_UPDATE: &[u8] = include_bytes!("fixtures/deck-schema-v3-connectors.update.bin");
const META: &str = "pptx:meta";
const SHAPE_ID: &str = "shape:4242:0";
const STORY_ID: &str = "story:shape:4242:0:0";

#[test]
fn released_v2_snapshot_migrates_without_changing_content_or_default_serialization() {
    assert_eq!(stamped_version(V2_STYLE_UPDATE), Some(2.0));
    let legacy_json = package_json(V2_STYLE_UPDATE);
    assert!(!legacy_json.contains("formatScheme"));
    let session = DeckSession::open_from_update(V2_STYLE_UPDATE, 909).unwrap();
    let snapshot = session.snapshot().unwrap();
    let story = snapshot.slides[0]
        .shapes
        .iter()
        .find_map(|s| s.text_stories.first())
        .unwrap();
    assert_eq!(
        session.story(&story.id).unwrap().plain_text(),
        "persisted-v2 Styled"
    );
    let migrated = session.encode_state_as_update_v1();
    assert_eq!(stamped_version(&migrated), Some(4.0));
    assert_eq!(package_json(&migrated), legacy_json);
    let reopened = DeckSession::open_from_update(&migrated, 910).unwrap();
    assert_eq!(reopened.snapshot().unwrap(), snapshot);
    assert_eq!(reopened.encode_state_as_update_v1().len(), migrated.len());

    let styled = DeckSession::open(
        include_bytes!("../../pptx-parse/tests/fixtures/style-matrix-deck.pptx"),
        911,
    )
    .unwrap();
    let update = styled.encode_state_as_update_v1();
    let restored = DeckSession::open_from_update(&update, 912).unwrap();
    assert_eq!(
        restored.package().themes[0].format_scheme,
        styled.package().themes[0].format_scheme
    );
    assert_eq!(
        restored.package().slides[0].shapes,
        styled.package().slides[0].shapes
    );
    assert_eq!(
        package_json(&restored.encode_state_as_update_v1()),
        package_json(&update)
    );
}

#[test]
fn released_v3_snapshot_migrates_without_changing_connector_ordinals() {
    assert_eq!(stamped_version(V3_UPDATE), Some(3.0));
    let session = DeckSession::open_from_update_with_source(V3_UPDATE, V2_SOURCE, 917).unwrap();
    assert!(session.package().models_connectors());
    let snapshot = session.snapshot().unwrap();
    assert_eq!(
        snapshot.slides[0]
            .shapes
            .iter()
            .map(|shape| shape.source_id)
            .collect::<Vec<_>>(),
        [2, 3, 4]
    );
    let migrated = session.encode_state_as_update_v1();
    assert_eq!(stamped_version(&migrated), Some(4.0));
    assert_eq!(package_json(&migrated), package_json(V3_UPDATE));
    let reopened = DeckSession::open_from_update_with_source(&migrated, V2_SOURCE, 918).unwrap();
    assert_eq!(reopened.snapshot().unwrap(), snapshot);
    assert_eq!(reopened.encode_state_as_update_v1(), migrated);
    assert_eq!(reopened.save().unwrap(), session.save().unwrap());
}

#[test]
fn released_v1_snapshot_migrates_and_round_trips_as_v4() {
    assert_eq!(stamped_version(V1_UPDATE), Some(1.0));

    let session = DeckSession::open_from_update(V1_UPDATE, 901).unwrap();
    assert_v1_content(&session);

    let migrated = session.encode_state_as_update_v1();
    assert_eq!(stamped_version(&migrated), Some(4.0));
    assert!(
        package_json(&migrated).contains("\"charts\""),
        "the migrated package must carry the v2 chart field"
    );
    assert!(
        !package_json(&migrated).contains("firstSlideNum"),
        "default numbering must preserve the v2 package representation"
    );

    let reopened = DeckSession::open_from_update(&migrated, 902).unwrap();
    assert_v1_content(&reopened);
    assert_eq!(
        snapshot_shape_ids(&session.snapshot().unwrap()),
        snapshot_shape_ids(&reopened.snapshot().unwrap())
    );
    assert_eq!(
        reopened.encode_state_as_update_v1().len(),
        migrated.len(),
        "reopening a v4 snapshot must not migrate again"
    );
}

#[test]
fn a_migrated_session_still_edits() {
    let session = DeckSession::open_from_update(V1_UPDATE, 903).unwrap();
    session
        .insert_text(
            &EditCtx::local("test"),
            STORY_ID,
            0,
            "re-",
            &TextStyle::default(),
        )
        .unwrap();
    assert_eq!(
        session.story(STORY_ID).unwrap().plain_text(),
        "re-edited persisted on v1"
    );
    let reopened =
        DeckSession::open_from_update(&session.encode_state_as_update_v1(), 904).unwrap();
    assert_eq!(
        reopened.story(STORY_ID).unwrap().plain_text(),
        "re-edited persisted on v1"
    );
}

#[test]
fn two_clients_migrating_the_same_v1_snapshot_converge() {
    let left = DeckSession::open_from_update(V1_UPDATE, 907).unwrap();
    let right = DeckSession::open_from_update(V1_UPDATE, 908).unwrap();

    right
        .apply_update_v1(&left.encode_state_as_update_v1())
        .unwrap();
    left.apply_update_v1(&right.encode_state_as_update_v1())
        .unwrap();

    assert_eq!(left.snapshot().unwrap(), right.snapshot().unwrap());
    assert_eq!(
        stamped_version(&left.encode_state_as_update_v1()),
        Some(4.0)
    );
    assert_eq!(
        package_json(&left.encode_state_as_update_v1()),
        package_json(&right.encode_state_as_update_v1())
    );
}

#[test]
fn a_fresh_seed_persists_the_connector_filter_in_schema_v4() {
    let session = DeckSession::open(V2_SOURCE, 909).unwrap();
    let update = session.encode_state_as_update_v1();
    assert_eq!(stamped_version(&update), Some(4.0));
    assert!(package_json(&update).contains("\"shapeElements\":\"withConnectors\""));
    let reopened = DeckSession::open_from_update_with_source(&update, V2_SOURCE, 910).unwrap();
    assert!(reopened.package().models_connectors());
    assert_eq!(reopened.snapshot().unwrap(), session.snapshot().unwrap());
    assert_eq!(reopened.save().unwrap(), session.save().unwrap());
}

#[test]
fn a_v2_snapshot_migrates_without_changing_its_package_or_shape_ids() {
    assert_eq!(stamped_version(V2_UPDATE), Some(2.0));
    let session = DeckSession::open_from_update(V2_UPDATE, 911).unwrap();
    let migrated = session.encode_state_as_update_v1();
    assert_eq!(stamped_version(&migrated), Some(4.0));
    assert_eq!(package_json(&migrated), package_json(V2_UPDATE));
    assert!(!session.package().models_connectors());
    let snapshot = session.snapshot().unwrap();
    assert_eq!(
        snapshot_shape_ids(&snapshot),
        ["slide:0:256:shape:0", "slide:0:256:shape:1"]
    );
    assert_eq!(snapshot.slides[0].shapes[1].source_id, 4);
    let reopened = DeckSession::open_from_update_with_source(&migrated, V2_SOURCE, 912).unwrap();
    assert_eq!(reopened.snapshot().unwrap(), snapshot);
    assert_eq!(reopened.encode_state_as_update_v1(), migrated);
    let cloned =
        DeckSession::from_package_with_source(session.package().clone(), V2_SOURCE, 913).unwrap();
    let reattached = DeckSession::open_from_update_with_source(
        &cloned.encode_state_as_update_v1(),
        V2_SOURCE,
        914,
    )
    .unwrap();
    assert_eq!(reattached.save().unwrap(), reopened.save().unwrap());
}

#[test]
fn v2_migration_converges_and_accepts_an_existing_peer_edit() {
    let left = DeckSession::open_from_update_with_source(V2_UPDATE, V2_SOURCE, 915).unwrap();
    let right = DeckSession::open_from_update_with_source(V2_UPDATE, V2_SOURCE, 916).unwrap();
    left.apply_update_v1(&right.encode_state_as_update_v1())
        .unwrap();
    right
        .apply_update_v1(&left.encode_state_as_update_v1())
        .unwrap();
    left.apply_update_v1(V2_MOVED_UPDATE).unwrap();
    right
        .apply_update_v1(&left.encode_state_as_update_v1())
        .unwrap();
    assert_eq!(left.snapshot().unwrap(), right.snapshot().unwrap());
    assert_eq!(left.snapshot().unwrap().slides[0].shapes[1].x, 952_500);
    assert_eq!(
        stamped_version(&left.encode_state_as_update_v1()),
        Some(4.0)
    );
    assert_eq!(left.save().unwrap(), right.save().unwrap());
    assert!(!left.package().models_connectors());
}

#[test]
fn unmigratable_schema_versions_stay_rejected() {
    for version in [0.0, 1.5, 5.0, 6.0] {
        assert!(
            matches!(
                DeckSession::open_from_update(&restamped(V1_UPDATE, Some(version)), 905),
                Err(EditError::InvalidState(message))
                    if message == "unsupported deck schema version"
            ),
            "schema version {version} must be rejected"
        );
    }
    assert!(matches!(
        DeckSession::open_from_update(&restamped(V1_UPDATE, None), 906),
        Err(EditError::InvalidState(message))
            if message == "unsupported deck schema version"
    ));
}

#[test]
fn default_numbering_uses_v4_and_omits_the_default() {
    let session = DeckSession::open(FIXTURE, 913).unwrap();
    let update = session.encode_state_as_update_v1();
    assert_eq!(stamped_version(&update), Some(4.0));
    assert!(!package_json(&update).contains("firstSlideNum"));
    let restored = DeckSession::open_from_update(&update, 914).unwrap();
    assert_eq!(restored.package().presentation.first_slide_num, 1);
    assert_eq!(
        restored.encode_state_vector_v1(),
        session.encode_state_vector_v1()
    );
    assert_eq!(
        package_json(&restored.encode_state_as_update_v1()),
        package_json(&update)
    );
}

#[test]
fn slide_number_offsets_use_v4_and_migrate_older_snapshots() {
    let session = DeckSession::open(NUMBERED_FIXTURE, 910).unwrap();
    let update = session.encode_state_as_update_v1();
    assert_eq!(stamped_version(&update), Some(4.0));
    assert!(package_json(&update).contains("\"firstSlideNum\":10"));
    for version in [1.0, 2.0, 3.0, 4.0] {
        let restored =
            DeckSession::open_from_update(&restamped(&update, Some(version)), 911).unwrap();
        assert_eq!(restored.package().presentation.first_slide_num, 10);
        assert_eq!(restored.snapshot().unwrap(), session.snapshot().unwrap());
        let migrated = restored.encode_state_as_update_v1();
        assert_eq!(stamped_version(&migrated), Some(4.0));
        assert_eq!(package_json(&migrated), package_json(&update));
        let reopened = DeckSession::open_from_update(&migrated, 912).unwrap();
        assert_eq!(
            reopened.encode_state_vector_v1(),
            restored.encode_state_vector_v1()
        );
    }
}

#[test]
fn current_main_v3_snapshots_migrate_to_v4() {
    for (update, connectors) in [(V3_CONNECTORS_UPDATE, true), (V3_NUMBERED_UPDATE, false)] {
        assert_eq!(stamped_version(update), Some(3.0));
        assert!(!package_json(update).contains("firstSlideNum"));
        let session = DeckSession::open_from_update(update, 917).unwrap();
        assert_eq!(session.package().models_connectors(), connectors);
        assert_eq!(session.package().presentation.first_slide_num, 1);
        let migrated = session.encode_state_as_update_v1();
        assert_eq!(stamped_version(&migrated), Some(4.0));
        assert_eq!(package_json(&migrated), package_json(update));
        let reopened = DeckSession::open_from_update(&migrated, 918).unwrap();
        assert_eq!(reopened.snapshot().unwrap(), session.snapshot().unwrap());
        assert_eq!(reopened.encode_state_as_update_v1(), migrated);
        if connectors {
            assert_eq!(session.snapshot().unwrap().slides[0].shapes.len(), 3);
        }
    }
}

fn assert_v1_content(session: &DeckSession) {
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.width_emu, 12_192_000);
    assert_eq!(snapshot.height_emu, 6_858_000);
    assert_eq!(session.package().presentation.first_slide_num, 1);
    assert_eq!(snapshot.slides.len(), 3);
    assert_eq!(snapshot.slides[0].id, "slide:0:256");
    assert!(
        snapshot_shape_ids(&snapshot)
            .iter()
            .any(|id| id == SHAPE_ID)
    );
    assert_eq!(
        session.story(STORY_ID).unwrap().plain_text(),
        "edited persisted on v1"
    );
    assert!(session.package().charts.is_empty());
}

fn snapshot_shape_ids(snapshot: &DeckSnapshot) -> Vec<String> {
    snapshot
        .slides
        .iter()
        .flat_map(|slide| slide.shapes.iter())
        .map(|shape| shape.id.clone())
        .collect()
}

fn hydrated(update: &[u8]) -> Doc {
    let doc = Doc::new();
    doc.transact_mut()
        .apply_update(Update::decode_v1(update).unwrap())
        .unwrap();
    doc
}

fn meta(doc: &Doc) -> MapRef {
    doc.transact().get_map(META).unwrap()
}

fn stamped_version(update: &[u8]) -> Option<f64> {
    let doc = hydrated(update);
    let meta = meta(&doc);
    match meta.get(&doc.transact(), "schemaVersion") {
        Some(Out::Any(Any::Number(value))) => Some(value),
        _ => None,
    }
}

fn package_json(update: &[u8]) -> String {
    let doc = hydrated(update);
    let meta = meta(&doc);
    match meta.get(&doc.transact(), "packageJson") {
        Some(Out::Any(Any::Buffer(bytes))) => String::from_utf8(bytes.to_vec()).unwrap(),
        _ => panic!("missing packageJson"),
    }
}

fn restamped(update: &[u8], version: Option<f64>) -> Vec<u8> {
    let doc = hydrated(update);
    let meta = meta(&doc);
    {
        let mut txn = doc.transact_mut();
        match version {
            Some(version) => {
                meta.insert(&mut txn, "schemaVersion", version);
            }
            None => {
                meta.remove(&mut txn, "schemaVersion");
            }
        }
    }
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}
