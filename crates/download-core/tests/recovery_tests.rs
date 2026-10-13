use download_core::model::{ConflictPolicy, JobSpec, RequestContext};
use download_core::store::JobStore;
use tempfile::tempdir;
use url::Url;

#[test]
fn rejects_duplicate_source_and_destination() {
    let directory = tempdir().unwrap();
    let store = JobStore::open(&directory.path().join("jobs.db")).unwrap();
    let spec = JobSpec {
        source: Url::parse("https://example.test/file").unwrap(),
        destination: "file.bin".into(),
        request_context: RequestContext::default(),
        conflict_policy: ConflictPolicy::Reject,
    };
    store.create_job(&spec).unwrap();
    assert!(store.create_job(&spec).is_err());
}

#[test]
fn creates_unique_ids_after_reopening_store() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("jobs.db");
    let spec = JobSpec {
        source: Url::parse("https://example.test/one").unwrap(),
        destination: "one.bin".into(),
        request_context: RequestContext::default(),
        conflict_policy: ConflictPolicy::Reject,
    };
    let first = JobStore::open(&path).unwrap().create_job(&spec).unwrap();
    let second = JobStore::open(&path)
        .unwrap()
        .create_job(&JobSpec { destination: "two.bin".into(), ..spec })
        .unwrap();
    assert_ne!(first, second);
}
