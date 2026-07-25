use zbus::zvariant::Endian;

use super::super::cursor::Cursor;
use super::super::limits::{PreflightError, StringBudget, MAX_SIGNATURE_DEPTH};
use super::super::signature::SignatureType;

#[test]
fn primitive_value_skips_consume_their_complete_wire_payloads() {
    let cases: &[(SignatureType, &[u8])] = &[
        (SignatureType::Basic(b'y'), &[7]),
        (SignatureType::Basic(b'n'), &[1, 0]),
        (SignatureType::Basic(b'q'), &[1, 0]),
        (SignatureType::Basic(b'x'), &[0; 8]),
        (SignatureType::Basic(b't'), &[0; 8]),
        (SignatureType::Basic(b'd'), &[0; 8]),
        (SignatureType::Basic(b'g'), &[1, b's', 0]),
    ];

    for (value_type, bytes) in cases {
        let mut cursor = Cursor::new(bytes, 0, Endian::Little);
        let mut budget = StringBudget::default();

        assert_eq!(cursor.skip_value(value_type, &mut budget, false, 0), Ok(()));
        assert!(cursor.is_finished(), "wire type was not fully consumed");
    }
}

#[test]
fn value_skip_accepts_exact_depth_and_rejects_the_next_level() {
    let value_type = SignatureType::Basic(b'y');
    let mut accepted = Cursor::new(&[1], 0, Endian::Little);
    let mut accepted_budget = StringBudget::default();
    assert_eq!(
        accepted.skip_value(
            &value_type,
            &mut accepted_budget,
            false,
            MAX_SIGNATURE_DEPTH
        ),
        Ok(())
    );

    let mut rejected = Cursor::new(&[1], 0, Endian::Little);
    let mut rejected_budget = StringBudget::default();
    assert_eq!(
        rejected.skip_value(
            &value_type,
            &mut rejected_budget,
            false,
            MAX_SIGNATURE_DEPTH + 1
        ),
        Err(PreflightError::LimitsExceeded(
            "Notify variant nesting is too deep"
        ))
    );
}

#[test]
fn nested_variants_enforce_the_value_depth_limit() {
    let mut bytes = Vec::new();
    for _ in 0..=MAX_SIGNATURE_DEPTH {
        bytes.extend_from_slice(&[1, b'v', 0]);
    }
    bytes.extend_from_slice(&[1, b'y', 0, 7]);

    let mut cursor = Cursor::new(&bytes, 0, Endian::Little);
    let mut budget = StringBudget::default();
    assert_eq!(
        cursor.skip_value(&SignatureType::Variant, &mut budget, false, 0),
        Err(PreflightError::LimitsExceeded(
            "Notify variant nesting is too deep"
        ))
    );
}

#[test]
fn nested_arrays_enforce_the_value_depth_limit() {
    let mut value_type = SignatureType::Basic(b'y');
    let mut bytes = vec![7_u8];
    // The innermost byte array is consumed in place, so one extra array reaches the guard
    for _ in 0..=MAX_SIGNATURE_DEPTH + 1 {
        let mut container = u32::try_from(bytes.len())
            .expect("nested array length")
            .to_le_bytes()
            .to_vec();
        container.extend(bytes);
        bytes = container;
        value_type = SignatureType::Array(Box::new(value_type));
    }

    let mut cursor = Cursor::new(&bytes, 0, Endian::Little);
    let mut budget = StringBudget::default();
    assert_eq!(
        cursor.skip_value(&value_type, &mut budget, false, 0),
        Err(PreflightError::LimitsExceeded(
            "Notify variant nesting is too deep"
        ))
    );
}

#[test]
fn nested_structures_enforce_the_value_depth_limit() {
    let mut value_type = SignatureType::Basic(b'y');
    for _ in 0..=MAX_SIGNATURE_DEPTH {
        value_type = SignatureType::Structure(vec![value_type]);
    }

    let mut cursor = Cursor::new(&[7], 0, Endian::Little);
    let mut budget = StringBudget::default();
    assert_eq!(
        cursor.skip_value(&value_type, &mut budget, false, 0),
        Err(PreflightError::LimitsExceeded(
            "Notify variant nesting is too deep"
        ))
    );
}
