use knowledge_garden::container_staging::{Identity, PartStore, ProjectionKind};
use std::io;
fn identity(generation: &str) -> Identity {
    Identity {
        source_id: format!("source-{}", "a".repeat(64)),
        version: "b".repeat(64),
        generation: generation.into(),
        kind: ProjectionKind::EmailMimePart,
        locator: "EMAIL part=1.2".into(),
    }
}
#[test]
fn committed_decoded_bytes_resume_and_reject_a_stale_generation() {
    let root = tempfile::tempdir().unwrap();
    let mut store = PartStore::open(root.path(), identity("generation-1")).unwrap();
    store.commit(0, b"first \xe2", || Ok(())).unwrap();
    drop(store);
    let mut resumed = PartStore::open(root.path(), identity("generation-1")).unwrap();
    assert_eq!(resumed.offset(), 7);
    resumed.commit(7, b"\x98\x83 last", || Ok(())).unwrap();
    let completed = resumed.finish(14, || Ok(())).unwrap();
    assert_eq!(
        std::fs::read(completed.path).unwrap(),
        "first ☃ last".as_bytes()
    );
    let stale = resumed.commit(14, b"bad", || Err(io::Error::other("source removed")));
    assert!(stale.is_err());
    let fresh = PartStore::open(root.path(), identity("generation-2")).unwrap();
    assert_eq!(fresh.offset(), 0);
}
#[test]
fn incomplete_or_invalid_utf8_cannot_be_finalized() {
    let root = tempfile::tempdir().unwrap();
    let mut store = PartStore::open(root.path(), identity("generation-1")).unwrap();
    store.commit(0, b"\xff", || Ok(())).unwrap();
    assert!(store.finish(1, || Ok(())).is_err());
    assert!(store.finish(2, || Ok(())).is_err());
}

#[test]
fn crash_fixture() {
    let Ok(path) = std::env::var("KG_TEST_CONTAINER_STAGE") else {
        return;
    };
    let mut store = PartStore::open(std::path::Path::new(&path), identity("generation-1")).unwrap();
    store.commit(0, b"durable ", || Ok(())).unwrap();
    std::process::abort();
}

#[test]
fn abrupt_process_exit_preserves_a_committed_chunk() {
    let root = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_fixture", "--nocapture"])
        .env("KG_TEST_CONTAINER_STAGE", root.path())
        .status()
        .unwrap();
    assert!(!status.success());
    let mut store = PartStore::open(root.path(), identity("generation-1")).unwrap();
    assert_eq!(store.offset(), 8);
    store.commit(8, b"text", || Ok(())).unwrap();
    let completed = store.finish(12, || Ok(())).unwrap();
    assert_eq!(std::fs::read(completed.path).unwrap(), b"durable text");
}

#[test]
fn projection_verifier_rejects_wrong_generation_paths_and_changed_bytes() {
    use knowledge_garden::container_staging::verified_projection;
    let root = tempfile::tempdir().unwrap();
    let first_identity = identity("generation-1");
    let mut store = PartStore::open(root.path(), first_identity.clone()).unwrap();
    store.commit(0, b"evidence", || Ok(())).unwrap();
    let projection = store.finish(8, || Ok(())).unwrap();
    assert!(verified_projection(root.path(), &first_identity, &projection.sha256, &projection.path).is_ok());
    assert!(verified_projection(root.path(), &identity("generation-2"), &projection.sha256, &projection.path).is_err());
    let other = root.path().join("other.txt"); std::fs::write(&other, b"evidence").unwrap();
    assert!(verified_projection(root.path(), &first_identity, &projection.sha256, &other).is_err());
    std::fs::write(&projection.path, b"modified").unwrap();
    assert!(verified_projection(root.path(), &first_identity, &projection.sha256, &projection.path).is_err());
}
