//! Config and theme file creation/reset logic.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use crate::paths::format_with_home;
use unixnotis_core::util;

use super::{log_line, ActionContext};

const DND_STATE_FILE: &str = "state.json";

pub fn ensure_config(ctx: &mut ActionContext) -> Result<()> {
    let config = Config::default();
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    let config_path = Config::default_config_path().map_err(|err| anyhow!(err.to_string()))?;

    log_line(
        ctx,
        format!("Config directory: {}", format_with_home(&config_dir)),
    );

    // Ensure the config directory exists before creating defaults.
    fs::create_dir_all(&config_dir).with_context(|| "failed to create config directory")?;

    if config_path.exists() {
        log_line(
            ctx,
            format!("Config file present: {}", format_with_home(&config_path)),
        );
    } else {
        // Write a default config.toml when missing so users have a base to edit.
        let config_toml =
            toml::to_string_pretty(&config).map_err(|err| anyhow!(err.to_string()))?;
        write_atomic(&config_path, &config_toml)
            .with_context(|| "failed to write config.toml")?;
        log_line(
            ctx,
            format!("Config file created: {}", format_with_home(&config_path)),
        );
    }

    let theme_paths = config
        .resolve_theme_paths()
        .map_err(|err| anyhow!(err.to_string()))?;

    let theme_entries = [
        ("base.css", &theme_paths.base_css),
        ("panel.css", &theme_paths.panel_css),
        ("popup.css", &theme_paths.popup_css),
        ("widgets.css", &theme_paths.widgets_css),
    ];

    let pre_existing = theme_entries
        .iter()
        .map(|(_, path)| path.exists())
        .collect::<Vec<_>>();

    config
        .ensure_theme_files(&theme_paths)
        .map_err(|err| anyhow!(err.to_string()))?;

    for ((name, path), existed) in theme_entries.iter().zip(pre_existing.iter()) {
        let status = if *existed { "present" } else { "created" };
        log_line(
            ctx,
            format!(
                "Theme file {}: {} ({})",
                name,
                status,
                format_with_home(path)
            ),
        );
    }

    Ok(())
}

pub fn reset_config(ctx: &mut ActionContext) -> Result<()> {
    let config = Config::default();
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    let config_path = Config::default_config_path().map_err(|err| anyhow!(err.to_string()))?;

    fs::create_dir_all(&config_dir).with_context(|| "failed to create config directory")?;

    // Preserve existing config before overwriting so customizations are recoverable.
    backup_existing_file(ctx, &config_path, "config.toml")?;

    let config_toml = toml::to_string_pretty(&config).map_err(|err| anyhow!(err.to_string()))?;
    write_atomic(&config_path, &config_toml).with_context(|| "failed to write config.toml")?;

    log_line(
        ctx,
        format!(
            "Reset config file to defaults: {}",
            format_with_home(&config_path)
        ),
    );

    let theme_paths = config
        .resolve_theme_paths()
        .map_err(|err| anyhow!(err.to_string()))?;

    // Backup theme files before reset to avoid accidental loss of user styling.
    backup_existing_file(ctx, &theme_paths.base_css, "base.css")?;
    backup_existing_file(ctx, &theme_paths.panel_css, "panel.css")?;
    backup_existing_file(ctx, &theme_paths.popup_css, "popup.css")?;
    backup_existing_file(ctx, &theme_paths.widgets_css, "widgets.css")?;

    write_atomic(&theme_paths.base_css, unixnotis_core::DEFAULT_BASE_CSS)
        .with_context(|| "failed to write base.css")?;
    write_atomic(&theme_paths.panel_css, unixnotis_core::DEFAULT_PANEL_CSS)
        .with_context(|| "failed to write panel.css")?;
    write_atomic(&theme_paths.popup_css, unixnotis_core::DEFAULT_POPUP_CSS)
        .with_context(|| "failed to write popup.css")?;
    write_atomic(&theme_paths.widgets_css, unixnotis_core::DEFAULT_WIDGETS_CSS)
        .with_context(|| "failed to write widgets.css")?;

    log_line(
        ctx,
        format!("Reset theme files in {}", format_with_home(&config_dir)),
    );

    Ok(())
}

pub fn remove_state(ctx: &mut ActionContext) -> Result<()> {
    let Some(state_dir) = resolve_state_dir() else {
        log_line(ctx, "State directory not resolved; skipping state cleanup.");
        return Ok(());
    };

    let state_root = state_dir.join("unixnotis");
    let outcome = match remove_state_file(&state_root) {
        Ok(outcome) => outcome,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            log_line(ctx, "State file not present; nothing to remove.");
            return Ok(());
        }
        Err(err) => return Err(err).with_context(|| "failed to remove state file"),
    };

    if outcome.removed_file {
        let path = state_root.join(DND_STATE_FILE);
        log_line(
            ctx,
            format!(
                "Removed persisted state file: {}",
                format_with_state_env(&path)
            ),
        );
    }

    if outcome.removed_dir {
        log_line(
            ctx,
            format!(
                "Removed empty state directory: {}",
                format_with_state_env(&state_root)
            ),
        );
    }

    Ok(())
}

#[derive(Debug, Default)]
struct RemoveStateOutcome {
    removed_file: bool,
    removed_dir: bool,
}

fn remove_state_file(state_root: &Path) -> std::io::Result<RemoveStateOutcome> {
    let state_file = state_root.join(DND_STATE_FILE);
    let removed_file = match fs::remove_file(&state_file) {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(err),
    };

    if !removed_file {
        return Ok(RemoveStateOutcome::default());
    }

    let removed_dir =
        is_dir_empty(state_root).unwrap_or(false) && fs::remove_dir(state_root).is_ok();

    Ok(RemoveStateOutcome {
        removed_file,
        removed_dir,
    })
}

fn is_dir_empty(path: &Path) -> std::io::Result<bool> {
    let mut entries = fs::read_dir(path)?;
    Ok(entries.next().is_none())
}

fn backup_existing_file(ctx: &mut ActionContext, path: &Path, label: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let backup_path = next_backup_path(path);
    // Copy first so the original remains intact until new content is written.
    // This avoids leaving users without a live config if a later write fails.
    fs::copy(path, &backup_path).with_context(|| format!("failed to backup {}", label))?;
    log_line(
        ctx,
        format!("Backed up {} to {}", label, format_with_home(&backup_path)),
    );
    Ok(())
}

fn next_backup_path(path: &Path) -> PathBuf {
    // Keep backups alongside the original file with .bak suffixes.
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let mut candidate = path.with_file_name(format!("{file_name}.bak"));
    if !candidate.exists() {
        return candidate;
    }

    // Increment suffixes to avoid overwriting prior backups.
    let mut index = 1;
    loop {
        candidate = path.with_file_name(format!("{file_name}.bak.{index}"));
        if !candidate.exists() {
            return candidate;
        }
        index += 1;
    }
}

fn resolve_state_dir() -> Option<PathBuf> {
    util::resolve_state_dir()
}

fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    // Write to a sibling temp file, then rename to avoid partial writes.
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp_name = format!("{file_name}.tmp-{}", std::process::id());
    let temp_path = path.with_file_name(temp_name);

    if temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }

    fs::write(&temp_path, contents)?;
    fs::rename(&temp_path, path).inspect_err(|_err| {
        let _ = fs::remove_file(&temp_path);
    })
}

fn format_with_state_env(path: &Path) -> String {
    // Prefer XDG_STATE_HOME for display when available to avoid absolute paths in logs.
    if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
        if !state_home.trim().is_empty() {
            let state_root = PathBuf::from(state_home);
            if let Ok(stripped) = path.strip_prefix(&state_root) {
                let mut rendered = PathBuf::from("$XDG_STATE_HOME");
                rendered.push(stripped);
                return rendered.display().to_string();
            }
        }
    }

    format_with_home(path)
}

#[cfg(test)]
mod tests {
    use super::{format_with_state_env, next_backup_path, remove_state_file, DND_STATE_FILE};
    use std::fs;
    use std::path::PathBuf;
    use unixnotis_core::util;

    #[test]
    fn next_backup_path_increments_suffixes() {
        // Ensures backup naming avoids overwriting existing .bak files.
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let dir = PathBuf::from(home).join(".cache").join(format!(
            "unixnotis-installer-backup-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let target = dir.join("config.toml");
        let _ = fs::write(&target, "original");
        let first = next_backup_path(&target);
        let _ = fs::write(&first, "bak");
        let second = next_backup_path(&target);
        assert_ne!(first, second);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_state_dir_prefers_xdg_state_home() {
        // Ensures explicit XDG_STATE_HOME is used when provided.
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        if home.trim().is_empty() {
            return;
        }
        let xdg = PathBuf::from(&home).join(".state-test");
        let dir = util::resolve_state_dir_from_env(
            Some(xdg.to_string_lossy().as_ref()),
            Some(home.as_str()),
        );
        assert_eq!(dir, Some(xdg));
    }

    #[test]
    fn resolve_state_dir_falls_back_to_home() {
        // Ensures HOME/.local/state is used when XDG_STATE_HOME is empty.
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        if home.trim().is_empty() {
            return;
        }
        let dir = util::resolve_state_dir_from_env(Some("  "), Some(home.as_str()));
        assert_eq!(dir, Some(PathBuf::from(&home).join(".local").join("state")));
    }

    #[test]
    fn remove_state_file_cleans_up_directory_when_empty() {
        // Confirms state.json removal cleans the directory when no other files exist.
        let root = PathBuf::from("target").join(format!(
            "unixnotis-installer-state-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&root);
        let state_path = root.join(DND_STATE_FILE);
        let _ = fs::write(&state_path, "{}");

        let outcome = remove_state_file(&root).expect("state removal should succeed");
        assert!(outcome.removed_file);
        assert!(!state_path.exists());
        assert!(outcome.removed_dir || !root.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn remove_state_file_keeps_directory_when_not_empty() {
        // Ensures unrelated files keep the state directory in place.
        let root = PathBuf::from("target").join(format!(
            "unixnotis-installer-state-nonempty-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&root);
        let state_path = root.join(DND_STATE_FILE);
        let other_path = root.join("extra.txt");
        let _ = fs::write(&state_path, "{}");
        let _ = fs::write(&other_path, "keep");

        let outcome = remove_state_file(&root).expect("state removal should succeed");
        assert!(outcome.removed_file);
        assert!(!state_path.exists());
        assert!(!outcome.removed_dir);
        assert!(root.exists());
        assert!(other_path.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn format_with_state_env_uses_xdg_state_home_prefix() {
        // Ensures state paths are rendered with $XDG_STATE_HOME when available.
        let key = "XDG_STATE_HOME";
        let original = std::env::var(key).ok();
        std::env::set_var(key, "state-root");

        let path = PathBuf::from("state-root")
            .join("unixnotis")
            .join(DND_STATE_FILE);
        let rendered = format_with_state_env(&path);
        assert!(rendered.starts_with("$XDG_STATE_HOME"));

        match original {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
