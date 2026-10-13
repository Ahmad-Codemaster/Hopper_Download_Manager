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
async fn unsupported_version_returns_structured_error() {
    let root = tempfile::tempdir().unwrap();
    let engine = Engine::new(root.path().to_path_buf()).unwrap();
    let response = route(&engine, CommandEnvelope {
        major: 2,
        minor: 0,
        command: Command::List,
    }).await;
    assert!(!response.ok);
    assert_eq!(response.payload["error"], "unsupported protocol version");
}
