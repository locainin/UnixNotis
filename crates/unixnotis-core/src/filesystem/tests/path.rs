//! Lexical and contained path tests

use std::path::{Path, PathBuf};

use proptest::prelude::*;

use crate::filesystem::{ContainedPath, LexicalPathError, LexicallyNormalizedPath};

#[test]
fn lexical_normalization_removes_current_and_internal_parent_components() {
    let path = LexicallyNormalizedPath::new("/srv/unixnotis/./scripts/old/../probe")
        .expect("normalize contained path");

    assert_eq!(path.as_path(), Path::new("/srv/unixnotis/scripts/probe"));
}

#[test]
fn lexical_normalization_can_transfer_owned_path_storage() {
    let path = LexicallyNormalizedPath::new("scripts/old/../probe")
        .expect("normalize owned path")
        .into_path_buf();

    assert_eq!(path, PathBuf::from("scripts/probe"));
}

#[test]
fn lexical_normalization_rejects_parent_escape() {
    assert_eq!(
        LexicallyNormalizedPath::new("../../outside"),
        Err(LexicalPathError::ParentEscape)
    );
    assert_eq!(
        LexicallyNormalizedPath::new("/../../outside"),
        Err(LexicalPathError::ParentEscape)
    );
}

#[test]
fn contained_paths_reject_absolute_and_relative_escape() {
    let root = Path::new("/srv/unixnotis");

    assert_eq!(
        ContainedPath::resolve_relative(root, "/tmp/outside"),
        Err(LexicalPathError::ExpectedRelative)
    );
    assert_eq!(
        ContainedPath::resolve_relative(root, "../outside"),
        Err(LexicalPathError::OutsideRoot)
    );
}

proptest! {
    #[test]
    fn normalization_is_idempotent(parts in prop::collection::vec("[a-z]{1,8}", 0..12)) {
        let path = parts.iter().collect::<PathBuf>();
        let once = LexicallyNormalizedPath::new(&path).expect("normalize generated path");
        let twice = LexicallyNormalizedPath::new(once.as_path()).expect("normalize normalized path");

        prop_assert_eq!(once, twice);
    }

    #[test]
    fn resolved_relative_paths_remain_beneath_root(
        parts in prop::collection::vec("[a-z]{1,8}", 0..12)
    ) {
        let relative = parts.iter().collect::<PathBuf>();
        let resolved = ContainedPath::resolve_relative("/srv/unixnotis", relative)
            .expect("resolve generated path");

        prop_assert!(resolved.absolute().starts_with(resolved.root()));
        prop_assert!(!resolved.relative().is_absolute());
    }
}
