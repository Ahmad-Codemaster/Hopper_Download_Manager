use download_core::model::{ConflictPolicy, JobSpec, JobState, RequestContext};
use download_core::store::JobStore;
use tempfile::tempdir;
use url::Url;

#[tokio::test]
async fn async_engine_downloads_and_publishes_completion() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 512];
        let _ = stream.read(&mut request);
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ndata").unwrap();
    });
    let root = tempdir().unwrap();
    let engine = download_core::engine::Engine::new(root.path().to_path_buf()).unwrap();
    let id = engine.create_job(JobSpec {
        source: Url::parse(&format!("http://{address}/file")).unwrap(),
        destination: "file.bin".into(),
        request_context: RequestContext::default(),
        conflict_policy: ConflictPolicy::Reject,
    }).await.unwrap();
    engine.start_job(id).await.unwrap();
    assert_eq!(std::fs::read(root.path().join("file.bin")).unwrap(), b"data");
}

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
