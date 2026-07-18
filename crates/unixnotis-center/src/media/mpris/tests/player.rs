use super::super::constants::{MPRIS_APP, MPRIS_PATH, MPRIS_PLAYER, MPRIS_PREFIX};
use super::super::player::owner_probe_is_stable;

#[test]
fn owner_probe_accepts_only_one_stable_unique_owner() {
    assert!(owner_probe_is_stable(":1.40", ":1.40"));
    assert!(!owner_probe_is_stable(":1.40", ":1.41"));
}

#[test]
fn player_proxy_constants_match_the_mpris_contract() {
    assert_eq!(MPRIS_PREFIX, "org.mpris.MediaPlayer2.");
    assert_eq!(MPRIS_PATH, "/org/mpris/MediaPlayer2");
    assert_eq!(MPRIS_PLAYER, "org.mpris.MediaPlayer2.Player");
    assert_eq!(MPRIS_APP, "org.mpris.MediaPlayer2");
}
