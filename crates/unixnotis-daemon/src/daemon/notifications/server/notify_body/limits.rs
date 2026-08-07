//! Limits and errors shared by raw Notify body readers

// Common native clients send decoded 1024x1024 RGBA application or contact images
pub(in crate::daemon::notifications::server) const MAX_NOTIFY_WIRE_IMAGE_BYTES: usize =
    4 * 1024 * 1024;
// Keep the wire geometry bound explicit even when a sparse row layout uses fewer bytes
pub(in crate::daemon::notifications::server) const MAX_NOTIFY_WIRE_IMAGE_DIMENSION: u32 = 1024;
// The image allowance plus bounded strings, actions, hints, and D-Bus alignment
pub(in crate::daemon::notifications::server) const MAX_NOTIFY_WIRE_BODY_BYTES: usize =
    MAX_NOTIFY_WIRE_IMAGE_BYTES + 128 * 1024;
pub(super) const MAX_NON_IMAGE_ARRAY_BYTES: usize = 16 * 1024;
pub(super) const MAX_NON_IMAGE_STRING_BYTES: usize = 64 * 1024;
pub(super) const MAX_NESTED_CONTAINER_ELEMENTS: usize = 64;
pub(super) const MAX_SIGNATURE_DEPTH: usize = 16;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::daemon::notifications::server) enum PreflightError {
    LimitsExceeded(&'static str),
    Malformed(&'static str),
}

#[derive(Default)]
pub(super) struct StringBudget {
    bytes: usize,
}

impl StringBudget {
    pub(super) fn add(&mut self, bytes: usize) -> Result<(), PreflightError> {
        // One cumulative budget prevents many valid strings from amplifying memory
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
