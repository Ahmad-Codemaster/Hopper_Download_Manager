use download_core::transfer::{validate_content_range, validate_range_response};

#[test]
fn accepts_matching_content_range() {
    assert_eq!(validate_content_range("bytes 0-9/10", 0, 9, 10), Ok(()));
}

#[test]
fn rejects_malformed_or_mismatched_content_range() {
    assert!(validate_content_range("bytes 0-8/10", 0, 9, 10).is_err());
    assert!(validate_content_range("not-a-range", 0, 9, 10).is_err());
}

#[test]
fn rejects_non_partial_range_response() {
    assert!(validate_range_response(
        reqwest::StatusCode::OK,
        Some("bytes 0-9/10"),
        0,
        9,
        10,
    )
    .is_err());
}

#[test]
fn rejects_invalid_range_response_header() {
    assert!(validate_range_response(
        reqwest::StatusCode::PARTIAL_CONTENT,
        Some("bytes 0-8/10"),
        0,
        9,
        10,
    )
    .is_err());
}
