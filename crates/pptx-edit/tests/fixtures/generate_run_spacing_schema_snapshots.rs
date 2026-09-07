use std::{env, fs, path::PathBuf};

use pptx_edit::DeckSession;
use yrs::{Any, Map, Out, ReadTxn, Transact};

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let root = PathBuf::from(&args[1]);
    let version: u32 = args[2].parse().unwrap();
    assert!(matches!(version, 17 | 18));
    for (source, name) in [
        (
            "crates/pptx-render/tests/fixtures/run-spacing.pptx",
            "run-spacing",
        ),
        (
            "crates/pptx-edit/tests/fixtures/run-spacing-shadow.pptx",
            "run-spacing-shadow",
        ),
    ] {
        let source = fs::read(root.join(source)).unwrap();
        let session = DeckSession::open(&source, 32500).unwrap();
        let txn = session.yrs_doc().transact();
        assert_eq!(
            txn.get_map("pptx:meta").unwrap().get(&txn, "schemaVersion"),
            Some(Out::Any(Any::Number(f64::from(version))))
        );
        let json = serde_json::to_string(session.package()).unwrap();
        assert!(!json.contains("spacingPt"));
        assert_eq!(
            json.contains("outerShadow"),
            version == 18 && name.ends_with("shadow")
        );
        let update = session.encode_state_as_update_v1();
        let reopened = DeckSession::open_from_update(&update, 32501).unwrap();
        assert_eq!(reopened.encode_state_as_update_v1(), update);
        fs::write(
            root.join(format!(
                "crates/pptx-edit/tests/fixtures/{name}-main-v{version}.update.bin"
            )),
            update,
        )
        .unwrap();
    }
}
