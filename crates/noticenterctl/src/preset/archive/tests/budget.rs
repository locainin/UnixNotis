use std::io::{Cursor, Read as _};

use super::super::budget::DecompressedBudget;

#[test]
fn decompressed_budget_accepts_input_at_the_exact_limit() {
    let input = vec![b'a'; 32];
    let mut bounded = DecompressedBudget::new(Cursor::new(input.clone()), 32);
    let mut output = Vec::new();

    bounded
        .read_to_end(&mut output)
        .expect("read input at exact budget");

    assert_eq!(output, input);
}

#[test]
fn decompressed_budget_rejects_the_first_byte_over_limit() {
    let mut bounded = DecompressedBudget::new(Cursor::new(vec![b'a'; 33]), 32);
    let mut output = Vec::new();

    let error = bounded
        .read_to_end(&mut output)
        .expect_err("reject input beyond budget");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("decompressed limit"));
    assert_eq!(output.len(), 32);
}
