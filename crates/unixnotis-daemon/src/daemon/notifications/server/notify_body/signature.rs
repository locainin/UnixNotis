//! Bounded parser for variant-contained D-Bus signatures

use super::limits::{PreflightError, MAX_NESTED_CONTAINER_ELEMENTS, MAX_SIGNATURE_DEPTH};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SignatureType {
    Basic(u8),
    Variant,
    Array(Box<Self>),
    Structure(Vec<Self>),
    DictEntry(Vec<Self>),
}

impl SignatureType {
    pub(super) const fn alignment(&self) -> usize {
        match self {
            Self::Basic(b'n' | b'q') => 2,
            Self::Basic(b'b' | b'i' | b'u' | b'h' | b's' | b'o') | Self::Array(_) => 4,
            Self::Basic(b'x' | b't' | b'd') | Self::Structure(_) | Self::DictEntry(_) => 8,
            Self::Basic(_) | Self::Variant => 1,
        }
    }
}

pub(super) struct SignatureParser<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SignatureParser<'a> {
    pub(super) fn one(bytes: &'a [u8]) -> Result<SignatureType, PreflightError> {
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
            // Container signatures are bounded independently from data element counts
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
