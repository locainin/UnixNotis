//! CSS loader path tests

use std::path::Path;

use super::*;

#[test]
fn normalize_lexical_path_preserves_leading_parent_segments() {
    let normalized = normalize_lexical_path(Path::new("../assets/./icons/../icon.png"));

    // Relative paths outside the base still keep the leading parent segment
    assert_eq!(normalized, Path::new("../assets/icon.png"));
}

#[test]
fn normalize_lexical_path_does_not_pop_past_root() {
    let normalized = normalize_lexical_path(Path::new("/tmp/../../icon.png"));

    // Absolute paths must never collapse into a relative path while folding parents
    assert_eq!(normalized, Path::new("/icon.png"));
}
