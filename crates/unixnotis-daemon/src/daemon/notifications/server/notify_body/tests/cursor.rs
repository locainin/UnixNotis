use zbus::zvariant::Endian;

use super::super::cursor::Cursor;
use super::super::limits::StringBudget;
use super::super::PreflightError;

#[test]
fn cursor_rejects_fixed_reads_past_the_body() {
    let mut cursor = Cursor::new(&[0_u8; 3], 0, Endian::Little);

    assert_eq!(
        cursor.read_fixed(4, 4),
        Err(PreflightError::Malformed("Notify body is truncated"))
    );
}

#[test]
fn cursor_rejects_string_without_nul_terminator() {
    let bytes = [1_u8, 0, 0, 0, b'x', b'!'];
    let mut cursor = Cursor::new(&bytes, 0, Endian::Little);
    let mut budget = StringBudget::default();

    assert_eq!(
        cursor.read_string(8, &mut budget),
        Err(PreflightError::Malformed(
            "Notify string is missing its terminator"
        ))
    );
}

#[test]
fn cursor_rejects_truncated_signature_and_bad_terminator() {
    let truncated = [2_u8, b'a'];
    let mut truncated_cursor = Cursor::new(&truncated, 0, Endian::Little);
    assert_eq!(
        truncated_cursor.read_signature(),
        Err(PreflightError::Malformed("Notify signature is truncated"))
    );

    let bad_terminator = [1_u8, b's', b'!'];
    let mut terminator_cursor = Cursor::new(&bad_terminator, 0, Endian::Little);
    assert_eq!(
        terminator_cursor.read_signature(),
        Err(PreflightError::Malformed(
            "Notify signature is missing its terminator"
        ))
    );
}

#[test]
fn cursor_reads_big_endian_u32_after_absolute_alignment() {
    let bytes = [0_u8, 0, 0, 0x01, 0x02, 0x03, 0x04];
    let mut cursor = Cursor::new(&bytes, 1, Endian::Big);

    assert_eq!(cursor.read_u32(), Ok(0x0102_0304));
    assert_eq!(cursor.position(), 7);
}

#[test]
fn cursor_rejects_array_length_beyond_remaining_bytes() {
    let bytes = [8_u8, 0, 0, 0, 1, 2, 3, 4];
    let mut cursor = Cursor::new(&bytes, 0, Endian::Little);

    assert_eq!(
        cursor.begin_array(4),
        Err(PreflightError::Malformed("Notify array is truncated"))
    );
}

#[test]
fn cursor_reports_completion_only_after_consuming_every_byte() {
    let mut cursor = Cursor::new(&[7_u8], 0, Endian::Little);

    assert!(!cursor.is_finished());
    assert_eq!(cursor.advance(1), Ok(()));
    assert!(cursor.is_finished());
}

#[test]
fn array_cursor_accepts_an_exact_body_and_rejects_element_mismatch() {
    let bytes = [4_u8, 0, 0, 0, 1, 2, 3, 4];
    let mut cursor = Cursor::new(&bytes, 0, Endian::Little);

    let end = cursor.begin_array(4).expect("exact array body");
    assert_eq!(end, bytes.len());
    assert_eq!(
        cursor.finish_array(end),
        Err(PreflightError::Malformed(
            "Notify array elements do not match its byte length"
        ))
    );

    cursor.advance(4).expect("consume array body");
    assert_eq!(cursor.finish_array(end), Ok(()));
}
