use download_host::ipc::{decode, encode, Envelope};

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
