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
/// Reject oversized property-change signals before dynamic value deserialization
pub const MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES: usize = MAX_MPRIS_PROPERTY_REPLY_BYTES;
/// Bound dictionary and invalidation entries after the encoded byte gate
pub const MAX_MPRIS_CHANGED_PROPERTIES: usize = 32;
/// Identity is shown in the panel but is never allowed to grow without bound
pub const MAX_MPRIS_IDENTITY_BYTES: usize = 512;
pub const MPRIS_TIMEOUT_QUARANTINE_AFTER: u8 = 3;
pub const MPRIS_TIMEOUT_QUARANTINE_MS: u64 = 5_000;

/// Discovery is capped so one bus connection cannot create unbounded state
pub const MAX_MPRIS_PLAYERS: usize = 32;
/// Quiet full-capacity inventories rotate one admission opportunity at this interval
pub const MPRIS_FAIRNESS_LEASE_MS: u64 = 5_000;
/// Failed candidate construction receives a bounded retry without resetting its lease
pub const MPRIS_FAIRNESS_RETRY_MS: u64 = 1_000;
/// Candidate owner probes are bounded before any full player construction
pub const MAX_MPRIS_CANDIDATES_PER_PASS: usize = 128;

/// Metadata maps are retained only when they remain reasonably small
pub const MAX_METADATA_ENTRIES: usize = 256;
