use super::super::limits::{PreflightError, MAX_SIGNATURE_DEPTH};
use super::super::signature::{SignatureParser, SignatureType};

#[test]
fn signature_parser_accepts_one_nested_dictionary_array() {
    assert_eq!(
        SignatureParser::one(b"a{sv}"),
        Ok(SignatureType::Array(Box::new(SignatureType::DictEntry(
            vec![SignatureType::Basic(b's'), SignatureType::Variant]
        ))))
    );
}

#[test]
fn signature_parser_rejects_empty_trailing_and_unknown_types() {
    assert_eq!(
        SignatureParser::one(b""),
        Err(PreflightError::Malformed(
            "Notify variant signature is empty"
        ))
    );
    assert_eq!(
        SignatureParser::one(b"ss"),
        Err(PreflightError::Malformed(
            "Notify variant signature has trailing types"
        ))
    );
    assert_eq!(
        SignatureParser::one(b"z"),
        Err(PreflightError::Malformed(
            "Notify variant signature contains an invalid type"
        ))
    );
}

#[test]
fn signature_parser_rejects_empty_unterminated_and_invalid_containers() {
    assert_eq!(
        SignatureParser::one(b"()"),
        Err(PreflightError::Malformed(
            "Notify container signature is empty"
        ))
    );
    assert_eq!(
        SignatureParser::one(b"(s"),
        Err(PreflightError::Malformed(
            "Notify container signature is unterminated"
        ))
    );
    assert_eq!(
        SignatureParser::one(b"{s}"),
        Err(PreflightError::Malformed(
            "Notify dictionary entry has an invalid signature"
        ))
    );
}

#[test]
fn signature_parser_rejects_nesting_beyond_the_depth_limit() {
    let mut signature = vec![b'a'; MAX_SIGNATURE_DEPTH + 2];
    signature.push(b'y');

    assert_eq!(
        SignatureParser::one(&signature),
        Err(PreflightError::LimitsExceeded(
            "Notify variant signature is too deep"
        ))
    );
}

#[test]
fn signature_parser_accepts_the_exact_depth_limit() {
    let mut signature = vec![b'a'; MAX_SIGNATURE_DEPTH];
    signature.push(b'y');

    assert!(SignatureParser::one(&signature).is_ok());
}

#[test]
fn signature_alignment_matches_each_dbus_wire_class() {
    assert_eq!(SignatureType::Basic(b'y').alignment(), 1);
    assert_eq!(SignatureType::Basic(b'n').alignment(), 2);
    assert_eq!(SignatureType::Basic(b'u').alignment(), 4);
    assert_eq!(SignatureType::Basic(b'x').alignment(), 8);
    assert_eq!(
        SignatureType::Array(Box::new(SignatureType::Basic(b'y'))).alignment(),
        4
    );
    assert_eq!(
        SignatureType::Structure(vec![SignatureType::Basic(b'y')]).alignment(),
        8
    );
}

#[test]
fn nested_structure_signatures_enforce_the_depth_limit() {
    let mut signature = vec![b'('; MAX_SIGNATURE_DEPTH + 2];
    signature.push(b'y');
    signature.extend(std::iter::repeat_n(b')', MAX_SIGNATURE_DEPTH + 2));

    assert_eq!(
        SignatureParser::one(&signature),
        Err(PreflightError::LimitsExceeded(
            "Notify variant signature is too deep"
        ))
    );
}
