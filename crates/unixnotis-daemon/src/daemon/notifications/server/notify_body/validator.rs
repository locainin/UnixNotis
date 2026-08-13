//! Validation flow for the fixed Notify D-Bus body shape

use zbus::Message;

use super::cursor::Cursor;
use super::limits::{PreflightError, StringBudget};
use super::signature::SignatureParser;
use crate::daemon::notifications::ingress::limits::{
    MAX_ACTIONS, MAX_ACTION_KEY_BYTES, MAX_ACTION_LABEL_BYTES, MAX_APP_ICON_BYTES,
    MAX_APP_NAME_BYTES, MAX_BODY_BYTES, MAX_HINT_ENTRIES, MAX_HINT_KEY_BYTES, MAX_SUMMARY_BYTES,
};

const NOTIFY_SIGNATURE: &str = "susssasa{sv}i";

pub(in crate::daemon::notifications::server) fn preflight_notify(
    message: &Message,
) -> Result<(), PreflightError> {
    let body = message.body();
    // The wire shape is checked before the typed interface creates owned containers
    if body
        .signature()
        .as_ref()
        .map(ToString::to_string)
        .as_deref()
        != Some(NOTIFY_SIGNATURE)
    {
        return Err(PreflightError::Malformed("Notify has an invalid signature"));
    }

    let data = body.data();
    let context = data.context();
    let mut cursor = Cursor::new(data.bytes(), context.position(), context.endian());
    let mut budget = StringBudget::default();

    // Fields follow the exact org.freedesktop.Notifications Notify order
    cursor.read_string(MAX_APP_NAME_BYTES, &mut budget)?;
    cursor.read_fixed(4, 4)?;
    cursor.read_string(MAX_APP_ICON_BYTES, &mut budget)?;
    cursor.read_string(MAX_SUMMARY_BYTES, &mut budget)?;
    cursor.read_string(MAX_BODY_BYTES, &mut budget)?;
    preflight_actions(&mut cursor, &mut budget)?;
    preflight_hints(&mut cursor, &mut budget)?;
    cursor.read_fixed(4, 4)?;
    if !cursor.is_finished() {
        return Err(PreflightError::Malformed("Notify body has trailing data"));
    }
    Ok(())
}

fn preflight_actions(
    cursor: &mut Cursor<'_>,
    budget: &mut StringBudget,
) -> Result<(), PreflightError> {
    let end = cursor.begin_array(4)?;
    let mut count = 0_usize;
    while cursor.position() < end {
        // Actions alternate key and label, with eight complete pairs allowed
        if count >= MAX_ACTIONS * 2 {
            return Err(PreflightError::LimitsExceeded(
                "Notify action array has too many elements",
            ));
        }
        let limit = if count.is_multiple_of(2) {
            MAX_ACTION_KEY_BYTES
        } else {
            MAX_ACTION_LABEL_BYTES
        };
        cursor.read_string(limit, budget)?;
        count += 1;
    }
    cursor.finish_array(end)
}

fn preflight_hints(
    cursor: &mut Cursor<'_>,
    budget: &mut StringBudget,
) -> Result<(), PreflightError> {
    let end = cursor.begin_array(8)?;
    let mut count = 0_usize;
    while cursor.position() < end {
        // Entry count is bounded before zbus can construct the owned map
        if count >= MAX_HINT_ENTRIES {
            return Err(PreflightError::LimitsExceeded(
                "Notify hint dictionary has too many entries",
            ));
        }
        cursor.align(8)?;
        let key = cursor.read_string(MAX_HINT_KEY_BYTES, budget)?;
        // Only standard image aliases receive the larger byte-array allowance
        let image_hint = matches!(key, b"image-data" | b"image_data" | b"icon_data");
        let signature = cursor.read_signature()?;
        let value_type = SignatureParser::one(signature)?;
        cursor.skip_value(&value_type, budget, image_hint, 0)?;
        count += 1;
    }
    cursor.finish_array(end)
}
