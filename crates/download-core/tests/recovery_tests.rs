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
