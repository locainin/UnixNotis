//! Recursive variant-value traversal without owned payload construction

use crate::daemon::notifications::ingress::limits::MAX_HINT_STRING_BYTES;

use super::cursor::Cursor;
use super::limits::{
    PreflightError, StringBudget, MAX_NESTED_CONTAINER_ELEMENTS, MAX_NON_IMAGE_ARRAY_BYTES,
    MAX_NOTIFY_WIRE_IMAGE_BYTES, MAX_SIGNATURE_DEPTH,
};
use super::signature::{SignatureParser, SignatureType};

impl Cursor<'_> {
    pub(super) fn skip_value(
        &mut self,
        value_type: &SignatureType,
        budget: &mut StringBudget,
        image_hint: bool,
        depth: usize,
    ) -> Result<(), PreflightError> {
        // Recursive variants and containers share one small depth limit
        if depth > MAX_SIGNATURE_DEPTH {
            return Err(PreflightError::LimitsExceeded(
                "Notify variant nesting is too deep",
            ));
        }
        match value_type {
            SignatureType::Basic(kind) => match kind {
                b'y' => self.advance(1),
                b'n' | b'q' => self.read_fixed(2, 2),
                b'b' | b'i' | b'u' | b'h' => self.read_fixed(4, 4),
                b'x' | b't' | b'd' => self.read_fixed(8, 8),
                b's' | b'o' => self.read_string(MAX_HINT_STRING_BYTES, budget).map(drop),
                b'g' => {
                    let signature = self.read_signature()?;
                    budget.add(signature.len())
                }
                _ => Err(PreflightError::Malformed(
                    "Notify variant has an unsupported basic type",
                )),
            },
            SignatureType::Variant => {
                let signature = self.read_signature()?;
                let nested = SignatureParser::one(signature)?;
                self.skip_value(&nested, budget, image_hint, depth + 1)
            }
            SignatureType::Array(element) => {
                let end = self.begin_array(element.alignment())?;
                if matches!(element.as_ref(), SignatureType::Basic(b'y')) {
                    // Raw bytes are skipped in place without constructing a vector
                    let length = self.remaining_to(end)?;
                    let limit = if image_hint {
                        MAX_NOTIFY_WIRE_IMAGE_BYTES
                    } else {
                        MAX_NON_IMAGE_ARRAY_BYTES
                    };
                    if length > limit {
                        return Err(PreflightError::LimitsExceeded(
                            "Notify byte array exceeds its allowance",
                        ));
                    }
                    self.finish_at(end);
                    return Ok(());
                }

                let mut count = 0_usize;
                while self.position() < end {
                    // Non-byte arrays receive an element cap as well as the wire-byte cap
                    if count >= MAX_NESTED_CONTAINER_ELEMENTS {
                        return Err(PreflightError::LimitsExceeded(
                            "Notify nested array has too many elements",
                        ));
                    }
                    self.skip_value(element, budget, image_hint, depth + 1)?;
                    count += 1;
                }
                self.finish_array(end)
            }
            SignatureType::Structure(fields) | SignatureType::DictEntry(fields) => {
                self.align(8)?;
                for field in fields {
                    self.skip_value(field, budget, image_hint, depth + 1)?;
                }
                Ok(())
            }
        }
    }
}
