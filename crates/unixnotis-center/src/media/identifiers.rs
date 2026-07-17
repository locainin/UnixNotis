//! Stable MPRIS names shared by discovery and proxy construction

// The prefix limits discovery to MPRIS-compatible session bus names
pub const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
// Every MPRIS application exposes its interfaces on this object path
pub const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
// Player controls and playback properties use the player interface
pub const MPRIS_PLAYER: &str = "org.mpris.MediaPlayer2.Player";
// Application identity and supported URI schemes use the root interface
pub const MPRIS_APP: &str = "org.mpris.MediaPlayer2";

#[cfg(test)]
#[path = "tests/identifiers.rs"]
mod tests;
