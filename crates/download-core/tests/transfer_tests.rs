use download_core::engine::{EngineConfig, TransferEngine};
use download_core::model::{ConflictPolicy, JobSpec, RequestContext};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use tempfile::tempdir;
use url::Url;

fn server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let body = b"downloaded";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Disposition: attachment; filename=\"file.bin\"\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
        }
    });
    (format!("http://{address}/file.bin"), handle)
}

#[test]
fn downloads_a_file_and_finalizes_it() {
    let (source, handle) = server();
    let root = tempdir().unwrap();
    let engine = TransferEngine::new(EngineConfig {
        destination_root: root.path().to_path_buf(),
    })
    .unwrap();
    let id = engine
        .create_job(JobSpec {
            source: Url::parse(&source).unwrap(),
            destination: "file.bin".into(),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Reject,
        })
        .unwrap();
    engine.start_job(id).unwrap();
    handle.join().unwrap();
    assert_eq!(std::fs::read(root.path().join("file.bin")).unwrap(), b"downloaded");
}

#[test]
fn rename_policy_does_not_overwrite_existing_file() {
    let (source, handle) = server();
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("file.bin"), b"existing").unwrap();
    let engine = TransferEngine::new(EngineConfig {
        destination_root: root.path().to_path_buf(),
    })
    .unwrap();
    let id = engine
        .create_job(JobSpec {
            source: Url::parse(&source).unwrap(),
            destination: "file.bin".into(),
            request_context: RequestContext::default(),
            conflict_policy: ConflictPolicy::Rename,
        })
        .unwrap();
    engine.start_job(id).unwrap();
    handle.join().unwrap();
    assert_eq!(std::fs::read(root.path().join("file.bin")).unwrap(), b"existing");
    assert_eq!(std::fs::read(root.path().join("file (1).bin")).unwrap(), b"downloaded");
}
