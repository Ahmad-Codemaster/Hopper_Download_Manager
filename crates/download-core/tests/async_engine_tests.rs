use download_core::model::{ConflictPolicy, JobSpec, JobState, RequestContext};
use download_core::store::JobStore;
use tempfile::tempdir;
use url::Url;

#[tokio::test]
async fn async_engine_downloads_and_publishes_completion() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use download_core::model::JobState;

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

    // Subscribe before starting so we don't miss the Completed event.
    let mut rx = engine.subscribe_events().unwrap();

    let id = engine.create_job(JobSpec {
        source: Url::parse(&format!("http://{address}/file")).unwrap(),
        destination: "file.bin".into(),
        request_context: RequestContext::default(),
        conflict_policy: ConflictPolicy::Reject,
    }).await.unwrap();

    // start_job returns immediately; download runs in background (CRIT-01 fix).
    engine.start_job(id.clone()).await.unwrap();

    // Wait for the Completed event (timeout after 5 s to avoid hanging CI).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let event = tokio::time::timeout(remaining, rx.recv()).await
            .expect("timed out waiting for download to complete")
            .expect("event channel closed");
        if event.job_id == id && event.state == JobState::Completed {
            break;
        }
    }

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

/// Regression test for CRIT-02: active_cancellations must be released when the
/// server responds with a non-2xx status so subsequent jobs are not permanently queued.
#[tokio::test]
async fn crit02_slot_released_after_http_error() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    // Spin up a minimal HTTP server that returns 404 for every request.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        // Handle up to 3 connections (max_concurrency default).
        for _ in 0..3 {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            }
        }
    });

    let root = tempfile::tempdir().unwrap();
    let engine = download_core::engine::Engine::new(root.path().to_path_buf()).unwrap();
    // Lower concurrency to 1 to make the test deterministic.
    engine.set_max_concurrent_jobs(1).unwrap();

    // First job: will fail with HTTP 404.
    let id_fail = engine
        .create_job(JobSpec {
            source: Url::parse(&format!("http://{addr}/fail")).unwrap(),
            destination: "fail.bin".into(),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Reject,
        })
        .await
        .unwrap();

    // start_job now returns immediately (spawns background task).
    engine.start_job(id_fail.clone()).await.unwrap();
    // Allow the background task to complete the failing request.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let jobs = engine.list_jobs().await.unwrap();
    let failed = jobs.iter().find(|j| j.id == id_fail).unwrap();
    assert_eq!(
        failed.state,
        JobState::Failed,
        "Job should be Failed after HTTP 404"
    );

    // If CRIT-02 is fixed the active slot is released; a new job must not immediately
    // be forced into Queued due to a leaked slot.
    let id2 = engine
        .create_job(JobSpec {
            source: Url::parse(&format!("http://{addr}/other")).unwrap(),
            destination: "other.bin".into(),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Reject,
        })
        .await
        .unwrap();

    engine.start_job(id2.clone()).await.unwrap();
    // Give the spawn a moment to register the job as Active (not Queued).
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let jobs2 = engine.list_jobs().await.unwrap();
    let job2 = jobs2.iter().find(|j| j.id == id2).unwrap();
    // The state should be Active or Failed (attempted), NOT Queued.
    assert_ne!(
        job2.state,
        JobState::Queued,
        "CRIT-02 regression: slot was leaked; new job stuck in Queued after previous 404"
    );
}

/// Regression test for CRIT-01: list_jobs must succeed while a slow download is
/// running (engine mutex must not be held for the entire transfer duration).
#[tokio::test]
async fn crit01_list_jobs_not_blocked_during_download() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    // Serve a 1-byte response with a deliberate 200 ms write delay to simulate a slow server.
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 512];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n");
            std::thread::sleep(std::time::Duration::from_millis(200));
            let _ = stream.write_all(b"X");
        }
    });

    let root = tempfile::tempdir().unwrap();
    let engine = download_core::engine::Engine::new(root.path().to_path_buf()).unwrap();

    let id = engine
        .create_job(JobSpec {
            source: Url::parse(&format!("http://{addr}/slow")).unwrap(),
            destination: "slow.bin".into(),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Reject,
        })
        .await
        .unwrap();

    // CRIT-01 fix: start_job must return immediately.
    engine.start_job(id).await.unwrap();

    // list_jobs should succeed without waiting for the download to finish.
    let jobs = engine.list_jobs().await.unwrap();
    assert_eq!(jobs.len(), 1, "list_jobs must return results while download is running");
}
