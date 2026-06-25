use std::path::Path;

/// Build the shared envdir setup used by Hyprland bootstrap commands
///
/// Each returned item is a shell fragment. Backends join them into one
/// `sh -lc` line because Hyprland startup entries are single command strings
pub(super) fn envdir_sync_prelude(env_dir: &Path) -> Vec<String> {
    let envdir = shell_quote_path(env_dir);

    vec![
        "umask 077".to_string(),
        format!("envdir={envdir}"),
        reject_symlinked_envdir(),
        create_envdir(),
        verify_real_envdir(),
    ]
}

/// Render one envdir file update for a selected environment variable
///
/// Missing variables intentionally create empty files. Both chpst and
/// s6-envdir treat empty envdir files as an unset request
pub(super) fn render_envdir_shell_update(name: &str) -> String {
    [
        format!("tmp=$(mktemp \"$envdir/.{name}.XXXXXX\") || exit"),
        format!("printenv {name} > \"$tmp\" || : > \"$tmp\""),
        "chmod 600 \"$tmp\" || { rm -f \"$tmp\"; exit 1; }".to_string(),
        format!("mv -f \"$tmp\" \"$envdir/{name}\" || {{ rm -f \"$tmp\"; exit 1; }}"),
    ]
    .join("; ")
}

/// Convert an env value into envdir file contents
///
/// Envdir readers only use the first line and trim trailing blanks. Matching
/// that behavior before writing avoids keeping stale shell noise
pub(super) fn envdir_file_contents(value: Option<&str>) -> String {
    value.map_or_else(String::new, |value| format!("{}\n", envdir_value(value)))
}

/// Return true when a variable name can safely become an envdir file name
pub(super) fn is_safe_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    // Keep names in ordinary shell-variable form so generated shell stays simple
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

pub(super) fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}

pub(super) fn shell_quote(raw: &str) -> String {
    if raw.is_empty() {
        return "''".to_string();
    }

    let mut quoted = String::with_capacity(raw.len() + 2);
    quoted.push('\'');
    for ch in raw.chars() {
        if ch == '\'' {
            // POSIX single-quote escape: close, emit escaped quote, reopen
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

fn reject_symlinked_envdir() -> String {
    r#"[ ! -L "$envdir" ] || exit 1"#.to_string()
}

fn create_envdir() -> String {
    r#"mkdir -p "$envdir" || exit 1"#.to_string()
}

fn verify_real_envdir() -> String {
    r#"[ -d "$envdir" ] && [ ! -L "$envdir" ] || exit 1"#.to_string()
}

fn envdir_value(value: &str) -> String {
    value
        .split(['\0', '\n'])
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '\t'])
        .to_string()
}
