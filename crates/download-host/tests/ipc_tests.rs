use download_host::ipc::{decode, encode, Envelope};
use download_host::ipc::{CommandEnvelope, Command};

#[test]
fn round_trips_versioned_envelope() {
    let envelope = Envelope::new("ping");
    assert_eq!(decode(&encode(&envelope).unwrap()).unwrap(), envelope);
}

#[test]
fn rejects_oversized_payload() {
    let envelope = Envelope::new(&"x".repeat(70_000));
    assert!(encode(&envelope).is_err());
}

#[test]
fn encodes_typed_list_command() {
    let command = CommandEnvelope::list();
    let decoded = CommandEnvelope::decode(&command.encode().unwrap()).unwrap();
    assert_eq!(decoded.command, Command::List);
}
