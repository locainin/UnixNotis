use super::has_valid_percent_encoding;

#[test]
fn percent_encoding_requires_two_hexadecimal_digits_after_every_marker() {
    for valid in [
        b"assets/icon.png".as_slice(),
        b"assets/icon%20one%23dark.png".as_slice(),
        b"%00".as_slice(),
    ] {
        assert!(
            has_valid_percent_encoding(valid),
            "{valid:?} should contain complete percent encoding"
        );
    }
    for invalid in [
        b"%".as_slice(),
        b"%2".as_slice(),
        b"%GG".as_slice(),
        b"%%20".as_slice(),
        b"%20%".as_slice(),
    ] {
        assert!(
            !has_valid_percent_encoding(invalid),
            "{invalid:?} should contain incomplete percent encoding"
        );
    }
}
