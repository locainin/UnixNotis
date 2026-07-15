use super::anchor_flags;
use unixnotis_core::Anchor;

#[test]
fn corner_anchors_enable_only_their_two_edges() {
    assert_eq!(anchor_flags(Anchor::TopRight), [true, true, false, false]);
    assert_eq!(anchor_flags(Anchor::BottomLeft), [false, false, true, true]);
}

#[test]
fn rail_anchors_enable_the_full_requested_axis() {
    assert_eq!(anchor_flags(Anchor::Top), [true, true, false, true]);
    assert_eq!(anchor_flags(Anchor::Right), [true, true, true, false]);
}
