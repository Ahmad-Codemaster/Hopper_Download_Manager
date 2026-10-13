use download_core::model::{ConflictPolicy, JobSpec, RequestContext};
use download_core::store::JobStore;
use tempfile::tempdir;
use url::Url;

#[test]
fn persists_job_and_checkpoint_across_reopen() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("jobs.db");
    let spec = JobSpec {
        source: Url::parse("https://example.test/file.bin?token=secret").unwrap(),
        destination: "file.bin".into(),
        request_context: RequestContext::default(),
        conflict_policy: ConflictPolicy::Reject,
    };
    let id = {
        let store = JobStore::open(&path).unwrap();
        let id = store.create_job(&spec).unwrap();
        store.update_checkpoint(id.clone(), 42).unwrap();
        id
    };
    let store = JobStore::open(&path).unwrap();
    let job = store.get_job(id).unwrap();
    assert_eq!(job.completed_bytes, 42);
    assert!(!job.source.contains("secret"));
}
