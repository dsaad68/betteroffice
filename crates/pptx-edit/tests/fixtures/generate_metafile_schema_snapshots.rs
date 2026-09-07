use std::{env, fs, path::PathBuf};

use pptx_edit::DeckSession;
use yrs::{Any, Map, Out, ReadTxn, Transact};

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let root = PathBuf::from(&args[1]);
    let version: u32 = args[2].parse().unwrap();
    assert!(matches!(version, 18 | 19));
    for (source, name) in [
        (
            "crates/pptx-render/tests/fixtures/metafile-pictures.pptx",
            "metafile-pictures",
        ),
        (
            "crates/pptx-edit/tests/fixtures/metafile-tracking.pptx",
            "metafile-tracking",
        ),
    ] {
        let session = DeckSession::open(&fs::read(root.join(source)).unwrap(), 31800).unwrap();
        let txn = session.yrs_doc().transact();
        assert_eq!(
            txn.get_map("pptx:meta").unwrap().get(&txn, "schemaVersion"),
            Some(Out::Any(Any::Number(f64::from(version))))
        );
        let snapshot = session.snapshot().unwrap();
        let graphic = snapshot.slides[0].shapes[2].graphic.as_ref().unwrap();
        assert!(
            serde_json::to_value(graphic)
                .unwrap()
                .get("picture")
                .is_none()
        );
        if name == "metafile-tracking" {
            let run = &snapshot.slides[0].shapes[4].text_stories[0].paragraphs[0].runs[0];
            assert_eq!(
                serde_json::to_value(&run.style)
                    .unwrap()
                    .get("spacingPt")
                    .cloned(),
                (version == 19).then(|| serde_json::json!(6.0))
            );
        }
        let update = session.encode_state_as_update_v1();
        let reopened = DeckSession::open_from_update(&update, 31801).unwrap();
        assert_eq!(reopened.encode_state_as_update_v1(), update);
        fs::write(
            root.join(format!(
                "crates/pptx-edit/tests/fixtures/{name}-v{version}.update.bin"
            )),
            update,
        )
        .unwrap();
    }
}
