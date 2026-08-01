//! Stable MPRIS names shared by discovery and proxy construction

// The prefix limits discovery to MPRIS-compatible session bus names
pub const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
// Every MPRIS application exposes its interfaces on this object path
pub const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
// Player controls and playback properties use the player interface
pub const MPRIS_PLAYER: &str = "org.mpris.MediaPlayer2.Player";
// Application identity and supported URI schemes use the root interface
pub const MPRIS_APP: &str = "org.mpris.MediaPlayer2";

/// Every untrusted MPRIS call must complete within one bounded interval
pub const MPRIS_PROPERTY_TIMEOUT_MS: u64 = 500;
/// Reject unusually large property replies before decoding dynamic values
pub const MAX_MPRIS_PROPERTY_REPLY_BYTES: usize = 512 * 1024;
/// Identity is shown in the panel but is never allowed to grow without bound
pub const MAX_MPRIS_IDENTITY_BYTES: usize = 512;
pub const MPRIS_TIMEOUT_QUARANTINE_AFTER: u8 = 3;
pub const MPRIS_TIMEOUT_QUARANTINE_MS: u64 = 5_000;

/// Discovery is capped so one bus connection cannot create unbounded state
pub const MAX_MPRIS_PLAYERS: usize = 32;

/// Metadata maps are retained only when they remain reasonably small
pub const MAX_METADATA_ENTRIES: usize = 256;
