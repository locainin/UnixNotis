use std::io::Cursor;

use super::skip_exact;

#[test]
fn skip_exact_consumes_only_the_requested_bytes() {
    let mut input = Cursor::new(b"abcdef".as_slice());

    skip_exact(&mut input, 3).expect("skip requested prefix");

    assert_eq!(input.position(), 3);
}
