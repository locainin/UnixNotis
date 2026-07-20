//! Machine-readable rfkill state parsing for the stock airplane toggle

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RfkillState {
    pub(super) device_count: usize,
    pub(super) all_soft_blocked: bool,
}

impl RfkillState {
    pub(super) const fn is_airplane_mode_active(self) -> bool {
        // Empty rfkill output never claims that airplane mode is active
        self.device_count > 0 && self.all_soft_blocked
    }
}

#[derive(Deserialize)]
struct RfkillDocument {
    rfkilldevices: Vec<RfkillDevice>,
}

#[derive(Deserialize)]
struct RfkillDevice {
    soft: String,
}

pub(super) fn parse_rfkill_state(output: &[u8]) -> Result<RfkillState, serde_json::Error> {
    // Structured output avoids localized and deprecated display formatting
    let document: RfkillDocument = serde_json::from_slice(output)?;
    Ok(RfkillState {
        device_count: document.rfkilldevices.len(),
        // Airplane mode requires every discovered radio to be soft blocked
        all_soft_blocked: document
            .rfkilldevices
            .iter()
            .all(|device| device.soft == "blocked"),
    })
}

#[cfg(test)]
#[path = "tests/rfkill.rs"]
mod tests;
