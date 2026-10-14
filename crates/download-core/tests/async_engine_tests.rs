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

#[tokio::test]
async fn engine_lists_jobs_and_exposes_lifecycle_controls() {
    let root = tempdir().unwrap();
    let engine = download_core::engine::Engine::new(root.path().to_path_buf()).unwrap();
    let id = engine.create_job(JobSpec {
        source: Url::parse("http://127.0.0.1/file").unwrap(),
        destination: "file.bin".into(),
        request_context: RequestContext::default(),
        conflict_policy: ConflictPolicy::Reject,
    }).await.unwrap();

    assert_eq!(engine.list_jobs().await.unwrap().len(), 1);
    engine.pause_job(id.clone()).await.unwrap();
    assert_eq!(engine.list_jobs().await.unwrap()[0].state, JobState::Paused);
    engine.resume_job(id.clone()).await.unwrap();
    engine.cancel_job(id).await.unwrap();
    assert_eq!(engine.list_jobs().await.unwrap()[0].state, JobState::Canceled);
}

#[tokio::test]
async fn test_refresh_expired_source_updates_store_and_memory() {
    let root = tempdir().unwrap();
    let engine = download_core::engine::Engine::new(root.path().to_path_buf()).unwrap();
    let id = engine
        .create_job(JobSpec {
            source: Url::parse("http://expired-link.test/file").unwrap(),
            destination: "file.bin".into(),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Reject,
        })
        .await
        .unwrap();

    let new_url = Url::parse("http://refreshed-link.test/file?token=secret123").unwrap();
    engine.refresh_source(id.clone(), new_url).await.unwrap();

    let jobs = engine.list_jobs().await.unwrap();
    assert_eq!(jobs[0].id, id);
    // The durable record is redacted to protect the refreshed credential
    assert!(jobs[0].source.contains("token=[REDACTED]"));
}

#[tokio::test]
async fn test_queue_concurrency_queues_excess_jobs() {
    let root = tempdir().unwrap();
    let engine = download_core::engine::Engine::new(root.path().to_path_buf()).unwrap();
    engine.set_max_concurrent_jobs(1).unwrap();
    assert_eq!(engine.get_max_concurrent_jobs().unwrap(), 1);

    let id1 = engine
        .create_job(JobSpec {
            source: Url::parse("http://127.0.0.1:9/never-connect-1").unwrap(),
            destination: "file1.bin".into(),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Reject,
        })
        .await
        .unwrap();

    let id2 = engine
        .create_job(JobSpec {
            source: Url::parse("http://127.0.0.1:9/never-connect-2").unwrap(),
            destination: "file2.bin".into(),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Reject,
        })
        .await
        .unwrap();

    // Spawn first job (it will attempt to connect and be active)
    let eng_clone = engine.clone();
    let id1_clone = id1.clone();
    tokio::spawn(async move {
        let _ = eng_clone.start_job(id1_clone).await;
    });

    // Give it a brief moment to register as active
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Start second job -> since concurrency is 1 and active is 1, it should queue
    engine.start_job(id2.clone()).await.unwrap();

    let jobs = engine.list_jobs().await.unwrap();
    let job2 = jobs.iter().find(|j| j.id == id2).unwrap();
    assert_eq!(job2.state, JobState::Queued);

    // Cancel job 1
    engine.cancel_job(id1).await.unwrap();
}


