//! Allocation-bounded structural preflight for the fixed Notify D-Bus body

use zbus::zvariant::Endian;
use zbus::Message;

use crate::daemon::notifications::limits::{
    MAX_ACTIONS, MAX_ACTION_KEY_BYTES, MAX_ACTION_LABEL_BYTES, MAX_APP_ICON_BYTES,
    MAX_APP_NAME_BYTES, MAX_BODY_BYTES, MAX_HINT_ENTRIES, MAX_HINT_KEY_BYTES,
    MAX_HINT_STRING_BYTES, MAX_SUMMARY_BYTES,
};

const NOTIFY_SIGNATURE: &str = "susssasa{sv}i";
const MAX_IMAGE_BYTES: usize = 256 * 1024;
const MAX_NON_IMAGE_ARRAY_BYTES: usize = 16 * 1024;
const MAX_NON_IMAGE_STRING_BYTES: usize = 64 * 1024;
const MAX_NESTED_CONTAINER_ELEMENTS: usize = 64;
const MAX_SIGNATURE_DEPTH: usize = 16;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum PreflightError {
    LimitsExceeded(&'static str),
    Malformed(&'static str),
}

pub(super) fn preflight_notify(message: &Message) -> Result<(), PreflightError> {
    let body = message.body();
    // The fixed wire shape is checked before the typed interface creates owned containers
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

    // Fields are consumed in the exact org.freedesktop.Notifications Notify order
    cursor.read_string(MAX_APP_NAME_BYTES, &mut budget)?;
    cursor.read_fixed(4, 4)?;
    cursor.read_string(MAX_APP_ICON_BYTES, &mut budget)?;
    cursor.read_string(MAX_SUMMARY_BYTES, &mut budget)?;
    cursor.read_string(MAX_BODY_BYTES, &mut budget)?;
    preflight_actions(&mut cursor, &mut budget)?;
    preflight_hints(&mut cursor, &mut budget)?;
    cursor.read_fixed(4, 4)?;
    if cursor.offset != cursor.bytes.len() {
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
    while cursor.offset < end {
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
    cursor.finish_array(end)?;
    Ok(())
}

fn preflight_hints(
    cursor: &mut Cursor<'_>,
    budget: &mut StringBudget,
) -> Result<(), PreflightError> {
    let end = cursor.begin_array(8)?;
    let mut count = 0_usize;
    while cursor.offset < end {
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
    cursor.finish_array(end)?;
    Ok(())
}

#[derive(Default)]
struct StringBudget {
    bytes: usize,
}

impl StringBudget {
    fn add(&mut self, bytes: usize) -> Result<(), PreflightError> {
        // One cumulative budget prevents many individually valid strings from amplifying memory
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(PreflightError::LimitsExceeded(
                "Notify string budget overflowed",
            ))?;
        if self.bytes > MAX_NON_IMAGE_STRING_BYTES {
            return Err(PreflightError::LimitsExceeded(
                "Notify contains too much non-image string data",
            ));
        }
        Ok(())
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    absolute_start: usize,
    endian: Endian,
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8], absolute_start: usize, endian: Endian) -> Self {
        Self {
            bytes,
            absolute_start,
            endian,
            offset: 0,
        }
    }

    fn align(&mut self, alignment: usize) -> Result<(), PreflightError> {
        // D-Bus alignment is relative to the whole message rather than this body slice
        let absolute = self
            .absolute_start
            .checked_add(self.offset)
            .ok_or(PreflightError::Malformed("Notify alignment overflowed"))?;
        let padding = (alignment - absolute % alignment) % alignment;
        self.advance(padding)
    }

    fn advance(&mut self, bytes: usize) -> Result<(), PreflightError> {
        // Checked offsets turn malformed lengths into errors instead of wraparound
        let end = self
            .offset
            .checked_add(bytes)
            .ok_or(PreflightError::Malformed("Notify offset overflowed"))?;
        if end > self.bytes.len() {
            return Err(PreflightError::Malformed("Notify body is truncated"));
        }
        self.offset = end;
        Ok(())
    }

    fn read_fixed(&mut self, alignment: usize, bytes: usize) -> Result<(), PreflightError> {
        self.align(alignment)?;
        self.advance(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, PreflightError> {
        let value = *self
            .bytes
            .get(self.offset)
            .ok_or(PreflightError::Malformed("Notify body is truncated"))?;
        self.offset += 1;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32, PreflightError> {
        self.align(4)?;
        let end = self
            .offset
            .checked_add(4)
            .ok_or(PreflightError::Malformed("Notify offset overflowed"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(PreflightError::Malformed("Notify body is truncated"))?;
        self.offset = end;
        Ok(self.endian.read_u32(bytes))
    }

    fn read_string(
        &mut self,
        limit: usize,
        budget: &mut StringBudget,
    ) -> Result<&'a [u8], PreflightError> {
        // Length is rejected before a slice is exposed to later parsing
        let length = usize::try_from(self.read_u32()?)
            .map_err(|_| PreflightError::LimitsExceeded("Notify string is too large"))?;
        if length > limit {
            return Err(PreflightError::LimitsExceeded(
                "Notify string exceeds its field limit",
            ));
        }
        budget.add(length)?;
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PreflightError::Malformed("Notify string offset overflowed"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(PreflightError::Malformed("Notify string is truncated"))?;
        self.offset = end;
        if self.read_u8()? != 0 {
            return Err(PreflightError::Malformed(
                "Notify string is missing its terminator",
            ));
        }
        Ok(value)
    }

    fn read_signature(&mut self) -> Result<&'a [u8], PreflightError> {
        let length = usize::from(self.read_u8()?);
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PreflightError::Malformed(
                "Notify signature offset overflowed",
            ))?;
        let signature = self
            .bytes
            .get(self.offset..end)
            .ok_or(PreflightError::Malformed("Notify signature is truncated"))?;
        self.offset = end;
        if self.read_u8()? != 0 {
            return Err(PreflightError::Malformed(
                "Notify signature is missing its terminator",
            ));
        }
        Ok(signature)
    }

    fn begin_array(&mut self, element_alignment: usize) -> Result<usize, PreflightError> {
        // Array byte lengths are validated before any element walk begins
        let length = usize::try_from(self.read_u32()?)
            .map_err(|_| PreflightError::LimitsExceeded("Notify array is too large"))?;
        self.align(element_alignment)?;
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PreflightError::Malformed("Notify array offset overflowed"))?;
        if end > self.bytes.len() {
            return Err(PreflightError::Malformed("Notify array is truncated"));
        }
        Ok(end)
    }

    fn finish_array(&self, end: usize) -> Result<(), PreflightError> {
        if self.offset == end {
            Ok(())
        } else {
            Err(PreflightError::Malformed(
                "Notify array elements do not match its byte length",
            ))
        }
    }

    fn skip_value(
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
                    // Raw bytes are skipped in place without constructing an intermediate vector
                    let length = end - self.offset;
                    let limit = if image_hint {
                        MAX_IMAGE_BYTES
                    } else {
                        MAX_NON_IMAGE_ARRAY_BYTES
                    };
                    if length > limit {
                        return Err(PreflightError::LimitsExceeded(
                            "Notify byte array exceeds its allowance",
                        ));
                    }
                    self.offset = end;
                    return Ok(());
                }
                let mut count = 0_usize;
                while self.offset < end {
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

#[derive(Debug)]
enum SignatureType {
    Basic(u8),
    Variant,
    Array(Box<SignatureType>),
    Structure(Vec<SignatureType>),
    DictEntry(Vec<SignatureType>),
}

impl SignatureType {
    const fn alignment(&self) -> usize {
        match self {
            Self::Basic(b'y' | b'g') | Self::Variant => 1,
            Self::Basic(b'n' | b'q') => 2,
            Self::Basic(b'b' | b'i' | b'u' | b'h' | b's' | b'o') | Self::Array(_) => 4,
            Self::Basic(b'x' | b't' | b'd') | Self::Structure(_) | Self::DictEntry(_) => 8,
            Self::Basic(_) => 1,
        }
    }
}

struct SignatureParser<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SignatureParser<'a> {
    fn one(bytes: &'a [u8]) -> Result<SignatureType, PreflightError> {
        // A variant signature must describe exactly one complete value
        let mut parser = Self { bytes, offset: 0 };
        let value_type = parser.parse_type(0)?;
        if parser.offset != bytes.len() {
            return Err(PreflightError::Malformed(
                "Notify variant signature has trailing types",
            ));
        }
        Ok(value_type)
    }

    fn parse_type(&mut self, depth: usize) -> Result<SignatureType, PreflightError> {
        // Parsing the tiny signature first makes the later byte walk deterministic
        if depth > MAX_SIGNATURE_DEPTH {
            return Err(PreflightError::LimitsExceeded(
                "Notify variant signature is too deep",
            ));
        }
        let kind = *self
            .bytes
            .get(self.offset)
            .ok_or(PreflightError::Malformed(
                "Notify variant signature is empty",
            ))?;
        self.offset += 1;
        match kind {
            b'y' | b'b' | b'n' | b'q' | b'i' | b'u' | b'x' | b't' | b'd' | b's' | b'o' | b'g'
            | b'h' => Ok(SignatureType::Basic(kind)),
            b'v' => Ok(SignatureType::Variant),
            b'a' => Ok(SignatureType::Array(Box::new(self.parse_type(depth + 1)?))),
            b'(' => self.parse_fields(b')', depth).map(SignatureType::Structure),
            b'{' => self.parse_fields(b'}', depth).and_then(|fields| {
                if fields.len() == 2 {
                    Ok(SignatureType::DictEntry(fields))
                } else {
                    Err(PreflightError::Malformed(
                        "Notify dictionary entry has an invalid signature",
                    ))
                }
            }),
            _ => Err(PreflightError::Malformed(
                "Notify variant signature contains an invalid type",
            )),
        }
    }

    fn parse_fields(
        &mut self,
        terminator: u8,
        depth: usize,
    ) -> Result<Vec<SignatureType>, PreflightError> {
        let mut fields = Vec::new();
        loop {
            // Container signatures are bounded independently from data-array element counts
            let Some(kind) = self.bytes.get(self.offset).copied() else {
                return Err(PreflightError::Malformed(
                    "Notify container signature is unterminated",
                ));
            };
            if kind == terminator {
                self.offset += 1;
                if fields.is_empty() {
                    return Err(PreflightError::Malformed(
                        "Notify container signature is empty",
                    ));
                }
                return Ok(fields);
            }
            if fields.len() >= MAX_NESTED_CONTAINER_ELEMENTS {
                return Err(PreflightError::LimitsExceeded(
                    "Notify container signature has too many fields",
                ));
            }
            fields.push(self.parse_type(depth + 1)?);
        }
    }
}

#[cfg(test)]
#[path = "tests/preflight.rs"]
mod tests;
