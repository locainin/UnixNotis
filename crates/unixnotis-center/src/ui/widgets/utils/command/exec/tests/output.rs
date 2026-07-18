use std::io::{self, Cursor};

use super::{read_to_end_limited, read_to_end_limited_async, MAX_CAPTURE_BYTES};

#[test]
fn blocking_reader_accepts_output_at_the_exact_limit() {
    let payload = vec![b'a'; MAX_CAPTURE_BYTES];

    let output = read_to_end_limited(Cursor::new(payload.clone())).expect("exact-limit output");

    assert_eq!(output, payload);
}

#[test]
fn blocking_reader_rejects_the_first_byte_over_the_limit() {
    let payload = vec![b'a'; MAX_CAPTURE_BYTES + 1];

    let error = read_to_end_limited(Cursor::new(payload)).expect_err("oversized output");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[tokio::test]
async fn async_reader_accepts_output_at_the_exact_limit() {
    let payload = vec![b'a'; MAX_CAPTURE_BYTES];

    let output = read_to_end_limited_async(payload.as_slice())
        .await
        .expect("exact-limit async output");

    assert_eq!(output, payload);
}

#[tokio::test]
async fn async_reader_rejects_the_first_byte_over_the_limit() {
    let payload = vec![b'a'; MAX_CAPTURE_BYTES + 1];

    let error = read_to_end_limited_async(payload.as_slice())
        .await
        .expect_err("oversized async output");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}
