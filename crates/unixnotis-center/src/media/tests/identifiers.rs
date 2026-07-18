use super::{MPRIS_APP, MPRIS_PATH, MPRIS_PLAYER, MPRIS_PREFIX};

#[test]
fn identifiers_match_the_mpris_interface_contract() {
    assert_eq!(MPRIS_PREFIX, "org.mpris.MediaPlayer2.");
    assert_eq!(MPRIS_PATH, "/org/mpris/MediaPlayer2");
    assert_eq!(MPRIS_PLAYER, "org.mpris.MediaPlayer2.Player");
    assert_eq!(MPRIS_APP, "org.mpris.MediaPlayer2");
}
