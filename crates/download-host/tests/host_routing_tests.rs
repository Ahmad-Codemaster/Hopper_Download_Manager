use download_core::engine::Engine;
use download_host::ipc::{Command, CommandEnvelope};
use download_host::routing::route;

#[tokio::test]
async fn list_command_returns_structured_success() {
    let root = tempfile::tempdir().unwrap();
    let engine = Engine::new(root.path().to_path_buf()).unwrap();
    let response = route(&engine, CommandEnvelope {
        major: 1,
        minor: 0,
        command: Command::List,
    }).await;
    assert!(response.ok);
    assert!(response.payload.is_array());
}

#[tokio::test]
async fn refresh_source_command_returns_success_for_valid_job() {
    let root = tempfile::tempdir().unwrap();
    let engine = Engine::new(root.path().to_path_buf()).unwrap();
    let id = engine.create_job(download_core::model::JobSpec {
        source: "http://example.com/file1".parse().unwrap(),
        destination: std::path::PathBuf::from("file.bin"),
        request_context: download_core::model::RequestContext::default(),
        conflict_policy: download_core::model::ConflictPolicy::Rename,
    }).await.unwrap();

    let response = route(&engine, CommandEnvelope {
        major: 1,
        minor: 0,
        command: Command::RefreshSource {
            job_id: id.0,
            new_source: "http://example.com/file2_new_token".into(),
        },
    }).await;
    assert!(response.ok);
}

