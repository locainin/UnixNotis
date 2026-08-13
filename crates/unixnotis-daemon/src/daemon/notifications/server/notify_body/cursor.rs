//! Checked byte cursor with D-Bus alignment and primitive readers

use zbus::zvariant::Endian;

use super::limits::{PreflightError, StringBudget};

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    absolute_start: usize,
    endian: Endian,
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(bytes: &'a [u8], absolute_start: usize, endian: Endian) -> Self {
        Self {
            bytes,
            absolute_start,
            endian,
            offset: 0,
        }
    }

    pub(super) const fn position(&self) -> usize {
        self.offset
    }

    pub(super) const fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    pub(super) fn align(&mut self, alignment: usize) -> Result<(), PreflightError> {
        // D-Bus alignment is relative to the whole message rather than this body slice
        let absolute = self
            .absolute_start
            .checked_add(self.offset)
            .ok_or(PreflightError::Malformed("Notify alignment overflowed"))?;
        let padding = (alignment - absolute % alignment) % alignment;
        self.advance(padding)
    }

    pub(super) fn advance(&mut self, bytes: usize) -> Result<(), PreflightError> {
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

    pub(super) fn read_fixed(
        &mut self,
        alignment: usize,
        bytes: usize,
    ) -> Result<(), PreflightError> {
        self.align(alignment)?;
        self.advance(bytes)
    }

    pub(super) fn read_u8(&mut self) -> Result<u8, PreflightError> {
        let value = *self
            .bytes
            .get(self.offset)
            .ok_or(PreflightError::Malformed("Notify body is truncated"))?;
        self.offset += 1;
        Ok(value)
    }

    pub(super) fn read_u32(&mut self) -> Result<u32, PreflightError> {
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

    pub(super) fn read_string(
        &mut self,
        limit: usize,
        budget: &mut StringBudget,
    ) -> Result<&'a [u8], PreflightError> {
        // Length is rejected before a slice is exposed to later parsing
        let length = usize::try_from(self.read_u32()?).map_err(|_conversion_error| {
            PreflightError::LimitsExceeded("Notify string is too large")
        })?;
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

    pub(super) fn read_signature(&mut self) -> Result<&'a [u8], PreflightError> {
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

    pub(super) fn begin_array(
        &mut self,
        element_alignment: usize,
    ) -> Result<usize, PreflightError> {
        // Array byte lengths are validated before any element walk begins
        let length = usize::try_from(self.read_u32()?).map_err(|_conversion_error| {
            PreflightError::LimitsExceeded("Notify array is too large")
        })?;
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

    pub(super) const fn finish_array(&self, end: usize) -> Result<(), PreflightError> {
        if self.offset == end {
            Ok(())
        } else {
            Err(PreflightError::Malformed(
                "Notify array elements do not match its byte length",
            ))
        }
    }

    pub(super) fn remaining_to(&self, end: usize) -> Result<usize, PreflightError> {
        end.checked_sub(self.offset)
            .ok_or(PreflightError::Malformed(
                "Notify array cursor passed its end",
            ))
    }

    pub(super) const fn finish_at(&mut self, end: usize) {
        self.offset = end;
    }
}
