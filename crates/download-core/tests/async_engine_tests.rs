use download_core::model::{ConflictPolicy, JobSpec, JobState, RequestContext};
use download_core::store::JobStore;
use tempfile::tempdir;
use url::Url;

#[test]
fn lists_jobs_and_reconciles_missing_completed_file() {
    let root = tempdir().unwrap();
    let store = JobStore::open(root.path()).unwrap();
    let spec = JobSpec {
        source: Url::parse("https://example.test/file").unwrap(),
        destination: "file.bin".into(),
        request_context: RequestContext::default(),
        conflict_policy: ConflictPolicy::Reject,
    };
    let id = store.create_job(&spec).unwrap();
    store.set_state(id.clone(), JobState::Completed).unwrap();
    let jobs = store.list_jobs().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].state, JobState::Failed);
}
