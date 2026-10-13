use download_host::native_messaging::{read_message, write_message};
use std::io::Cursor;

#[test]
fn frames_native_message_with_little_endian_length() {
    let mut bytes = Vec::new();
    write_message(&mut bytes, b"hello").unwrap();
    assert_eq!(bytes[..4], 5u32.to_le_bytes());
    assert_eq!(read_message(&mut Cursor::new(bytes)).unwrap(), b"hello");
}

#[test]
fn malformed_message_does_not_stop_following_valid_message() {
    let mut input = Vec::new();
    write_message(&mut input, b"{bad").unwrap();
    write_message(&mut input, br#"{"major":1,"minor":0,"payload":"ok"}"#).unwrap();
    let mut output = Vec::new();
    download_host::native_messaging::process_stream(
        &mut Cursor::new(input),
        &mut output,
    ).unwrap();
    assert!(output.len() > 8);
}
