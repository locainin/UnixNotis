//! Desktop identifiers, aliases, and protected brand normalization

use std::path::Path;

use unicode_security::skeleton;

pub(in crate::daemon::notifications::identity) fn is_shared_launcher(program: &Path) -> bool {
    let Some(name) = program.file_name().and_then(|value| value.to_str()) else {
        return true;
    };
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "sh" | "bash"
            | "dash"
            | "zsh"
            | "fish"
            | "env"
            | "node"
            | "nodejs"
            | "java"
            | "electron"
            | "wine"
            | "wine64"
            | "flatpak"
            | "gtk-launch"
            | "perl"
            | "ruby"
            | "php"
            | "lua"
            | "deno"
            | "bun"
    ) || name.strip_prefix("python").is_some_and(|suffix| {
        suffix
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
    })
}

pub(in crate::daemon::notifications::identity) fn normalize_desktop_id(value: &str) -> String {
    // Desktop hints commonly include an optional suffix and mixed case
    value
        .trim()
        .strip_suffix(".desktop")
        .unwrap_or_else(|| value.trim())
        .to_ascii_lowercase()
}

pub(in crate::daemon::notifications::identity) fn normalize_name(value: &str) -> String {
    // Punctuation and case do not create separate branding aliases
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn normalize_brand_name(value: &str) -> String {
    // UTS 39 skeletons collapse common cross-script lookalikes before comparison
    skeleton(value)
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}
