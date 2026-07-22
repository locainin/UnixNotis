//! Descriptor traversal policy tests

use rustix::fs::ResolveFlags;

use super::{anchor_resolve_flags, contained_resolve_flags};

#[test]
fn contained_resolution_policy_keeps_every_escape_barrier() {
    let flags = contained_resolve_flags();

    assert!(flags.contains(ResolveFlags::BENEATH));
    assert!(flags.contains(ResolveFlags::NO_SYMLINKS));
    assert!(flags.contains(ResolveFlags::NO_MAGICLINKS));
}

#[test]
fn anchor_resolution_policy_rejects_link_detours() {
    let flags = anchor_resolve_flags();

    assert!(flags.contains(ResolveFlags::NO_SYMLINKS));
    assert!(flags.contains(ResolveFlags::NO_MAGICLINKS));
    assert!(!flags.contains(ResolveFlags::BENEATH));
}
