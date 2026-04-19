//! Session environment synchronization helpers.
//!
//! Keeps user-session environment propagation separate from service lifecycle work
//! so install steps can import fresh variables before they start or restart units.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};

use crate::paths::format_with_home;
use unixnotis_core::program_in_path;

use super::{log_line, run_command, ActionContext};

pub(super) const HYPR_IMPORT_VARS: [&str; 7] = [
    "WAYLAND_DISPLAY",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "XDG_SESSION_DESKTOP",
    "DISPLAY",
    "XDG_RUNTIME_DIR",
    "PATH",
];
pub(super) const HYPR_REQUIRED_VARS: [&str; 2] = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"];
const PATH_BLOCK_MARKER: &str = "# unixnotis-installer path entry";

pub(super) fn sync_user_environment(ctx: &mut ActionContext) -> Result<()> {
    // This step only updates manager env
    // Service start or restart is handled by the caller
    if !program_in_path("systemctl") {
        let message = "systemctl not found; cannot sync user environment";
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    }

    // Require a minimal Wayland session context before attempting import.
    let missing_required = HYPR_REQUIRED_VARS
        .iter()
        .copied()
        .filter(|var| env::var(var).is_err())
        .collect::<Vec<_>>();
    if !missing_required.is_empty() {
        let message = format!(
            "missing session variables: {}; run from a Wayland session",
            missing_required.join(", ")
        );
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    }

    // Track whether any environment sync step completed successfully.
    let mut updated = false;
    if program_in_path("dbus-update-activation-environment") {
        // Prefer dbus-update-activation-environment to keep systemd --user in sync with the session.
        let mut command = Command::new("dbus-update-activation-environment");
        command.args(["--systemd", "--all"]);
        if let Err(err) = run_command(
            ctx,
            "dbus-update-activation-environment --systemd --all",
            command,
            None,
        ) {
            log_line(ctx, format!("Warning: {}", err));
        } else {
            updated = true;
        }
    } else {
        log_line(
            ctx,
            "Warning: dbus-update-activation-environment not found; session env may be stale",
        );
    }

    // Import session variables that are commonly missing from systemd --user.
    let vars = HYPR_IMPORT_VARS
        .iter()
        .copied()
        .filter(|var| env::var(var).is_ok())
        .collect::<Vec<_>>();
    if vars.is_empty() {
        let message = "no session environment variables found to import for systemd --user";
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    } else {
        let mut command = Command::new("systemctl");
        command.args(["--user", "--no-pager", "import-environment"]);
        command.args(&vars);
        if let Err(err) = run_command(
            ctx,
            "systemctl --user --no-pager import-environment",
            command,
            None,
        ) {
            log_line(ctx, format!("Warning: {}", err));
        } else {
            updated = true;
        }
    }

    if !updated {
        let message = "failed to synchronize systemd --user environment";
        log_line(ctx, format!("Error: {}", message));
        return Err(anyhow!(message));
    }

    // Service start or restart is owned by the caller so install flows do not boot
    // the daemon twice after environment import.
    Ok(())
}

pub(super) fn ensure_shell_path_entry(ctx: &mut ActionContext) -> Result<()> {
    // Startup file updates only affect new terminals
    let home = crate::paths::home_dir()?;
    let shell = env::var("SHELL").ok();
    let startup_files = shell_startup_files(&home, shell.as_deref());
    let mut updated_files = Vec::new();

    for startup_file in startup_files {
        if ensure_path_entry_in_file(&startup_file, &home, &ctx.paths.bin_dir)? {
            updated_files.push(startup_file);
        }
    }

    let rendered_bin = format_path_for_shell_line(&home, &ctx.paths.bin_dir);
    if updated_files.is_empty() {
        log_line(
            ctx,
            format!("Shell startup already includes PATH entry for {rendered_bin}"),
        );
    } else {
        for startup_file in updated_files {
            log_line(
                ctx,
                format!(
                    "Added PATH entry to {} so new terminals can run noticenterctl",
                    format_with_home(&startup_file)
                ),
            );
        }
    }

    Ok(())
}

fn shell_startup_files(home: &Path, shell: Option<&str>) -> Vec<PathBuf> {
    // Update shell rc first, then profile as a fallback
    let mut files = Vec::new();
    let mut push_unique = |path: PathBuf| {
        if !files.contains(&path) {
            files.push(path);
        }
    };

    match shell.unwrap_or_default() {
        s if s.ends_with("zsh") => push_unique(home.join(".zshrc")),
        s if s.ends_with("bash") => push_unique(home.join(".bashrc")),
        _ => {}
    }

    push_unique(home.join(".profile"));
    files
}

fn ensure_path_entry_in_file(file: &Path, home: &Path, bin_dir: &Path) -> Result<bool> {
    let existing = match fs::read_to_string(file) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(anyhow!("failed to read {}: {}", file.display(), err)),
    };

    if existing.contains(PATH_BLOCK_MARKER) || shell_path_entry_exists(&existing, home, bin_dir) {
        return Ok(false);
    }

    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| anyhow!("failed to create {}: {}", parent.display(), err))?;
    }

    let export_line = format!(
        "export PATH=\"{}:$PATH\"",
        format_path_for_shell_line(home, bin_dir)
    );
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(PATH_BLOCK_MARKER);
    updated.push('\n');
    updated.push_str(&export_line);
    updated.push('\n');

    fs::write(file, updated)
        .map_err(|err| anyhow!("failed to write {}: {}", file.display(), err))?;
    Ok(true)
}

fn shell_path_entry_exists(contents: &str, home: &Path, bin_dir: &Path) -> bool {
    let shell_path = format_path_for_shell_line(home, bin_dir);
    let absolute_path = bin_dir.display().to_string();

    contents.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("export PATH=")
            && (trimmed.contains(&shell_path) || trimmed.contains(&absolute_path))
    })
}

fn format_path_for_shell_line(home: &Path, bin_dir: &Path) -> String {
    // Keep startup files portable when path lives under HOME
    if let Ok(stripped) = bin_dir.strip_prefix(home) {
        let tail = stripped.to_string_lossy();
        if tail.is_empty() {
            "$HOME".to_string()
        } else {
            format!("$HOME/{}", tail.trim_start_matches('/'))
        }
    } else {
        bin_dir.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        ensure_path_entry_in_file, format_path_for_shell_line, shell_path_entry_exists,
        shell_startup_files,
    };

    #[test]
    fn shell_startup_files_prefers_zsh_and_profile() {
        let home = std::path::PathBuf::from("/tmp/unixnotis-home");
        let files = shell_startup_files(&home, Some("/usr/bin/zsh"));
        assert_eq!(files, vec![home.join(".zshrc"), home.join(".profile")]);
    }

    #[test]
    fn ensure_path_entry_in_file_is_idempotent() {
        let root = test_root("path-entry-idempotent");
        let home = root.join("home");
        let bin_dir = home.join(".local").join("bin");
        let startup = home.join(".zshrc");

        fs::create_dir_all(&home).expect("create home");
        let first = ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("first write");
        let second = ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("second write");
        let contents = fs::read_to_string(&startup).expect("read startup");
        assert!(first);
        assert!(!second);
        assert!(shell_path_entry_exists(&contents, &home, &bin_dir));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn format_path_for_shell_line_uses_home_prefix_when_possible() {
        let home = std::path::PathBuf::from("/tmp/unixnotis-home");
        let bin_dir = home.join(".local").join("bin");
        assert_eq!(
            format_path_for_shell_line(&home, &bin_dir),
            "$HOME/.local/bin"
        );
    }

    fn test_root(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "unixnotis-installer-env-{name}-{}-{stamp}",
            std::process::id()
        ))
    }
}
