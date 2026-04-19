//! Install and uninstall filesystem assets.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;

use anyhow::{anyhow, Context, Result};

use crate::paths::{format_with_home, InstallPaths};

use super::{
    actions_binaries::{
        resolve_install_binaries, resolve_install_binaries_best_effort, resolve_target_directory,
    },
    actions_config_backup::write_atomic,
    actions_env::{ensure_shell_path_entry, sync_user_environment},
    actions_hyprland::{ensure_hyprland_autostart, remove_hyprland_autostart},
    log_line, run_command, ActionContext,
};

pub fn install_binaries(ctx: &mut ActionContext) -> Result<()> {
    // Read the installable binary list from cargo metadata so install and uninstall stay in sync
    let binaries = resolve_install_binaries(ctx.paths)?;
    // Stop before touching ~/.local/bin if cargo cannot point at the real target dir
    let release_dir = resolve_release_dir(ctx)?;

    fs::create_dir_all(&ctx.paths.bin_dir).with_context(|| "failed to create bin directory")?;

    // Check every source binary first so install never leaves a half-updated bin dir
    let mut missing = Vec::new();
    for binary in &binaries {
        let source = release_dir.join(binary);
        if !source.exists() {
            missing.push(format_with_home(&source));
        }
    }
    if !missing.is_empty() {
        return Err(anyhow!(
            "missing build artifacts (aborting before install): {}",
            missing.join(", ")
        ));
    }

    for binary in binaries {
        let source = release_dir.join(&binary);
        let destination = ctx.paths.bin_dir.join(&binary);
        copy_binary(ctx, &source, &destination)?;
    }

    Ok(())
}

pub fn install_service(ctx: &mut ActionContext) -> Result<()> {
    fs::create_dir_all(&ctx.paths.unit_dir)
        .with_context(|| "failed to create systemd user directory")?;

    match write_service_unit(ctx)? {
        ServiceUnitWrite::CreatedOrUpdated => {
            log_line(
                ctx,
                format!(
                    "Installed systemd unit to {}",
                    format_with_home(&ctx.paths.unit_path)
                ),
            );
        }
        ServiceUnitWrite::Unchanged => {
            log_line(ctx, "Systemd unit already up to date");
        }
    }

    Ok(())
}

pub fn enable_service(ctx: &mut ActionContext) -> Result<()> {
    if ctx.service_unit_reload_required.load(Ordering::Acquire) {
        // A full user-manager reload is expensive on some setups, so only run
        // it when install actually changed the unixnotis unit file.
        log_line(ctx, "Reloading systemd user manager");
        let mut daemon_reload = Command::new("systemctl");
        daemon_reload.args(["--user", "daemon-reload"]);
        run_command(ctx, "systemctl --user daemon-reload", daemon_reload, None)?;
    } else {
        log_line(
            ctx,
            "Skipping systemd user reload because unit is unchanged",
        );
    }
    // Import the live session env first so the first service start picks it up.
    // This avoids an extra restart that can launch the full UI tree twice during install.
    sync_user_environment(ctx)?;
    run_service_start(ctx)?;
    // Startup files are updated so new terminals can resolve commands
    if let Err(err) = ensure_shell_path_entry(ctx) {
        log_line(
            ctx,
            format!("Warning: failed to update shell PATH files ({err})"),
        );
    }
    // Hyprland exec-once ensures session vars are synced once per login without extra hooks.
    ensure_hyprland_autostart(ctx);
    Ok(())
}

pub fn uninstall_service(ctx: &mut ActionContext) -> Result<()> {
    let unit = &ctx.paths.unit_path;
    let unit_display = format_with_home(unit);

    if unit.exists() {
        let mut disable = Command::new("systemctl");
        disable.args(["--user", "disable", "--now", "unixnotis-daemon.service"]);
        if let Err(err) = run_command(
            ctx,
            "systemctl --user disable --now unixnotis-daemon.service",
            disable,
            None,
        ) {
            log_line(ctx, format!("Warning: {}", err));
        }
        let mut daemon_reload = Command::new("systemctl");
        daemon_reload.args(["--user", "daemon-reload"]);
        fs::remove_file(unit).with_context(|| "failed to remove systemd unit")?;
        run_command(ctx, "systemctl --user daemon-reload", daemon_reload, None)?;
        log_line(ctx, format!("Removed systemd unit at {}", unit_display));
    } else {
        log_line(ctx, format!("Systemd unit not found at {}", unit_display));
    }

    remove_hyprland_autostart(ctx);
    Ok(())
}

pub fn remove_binaries(ctx: &mut ActionContext) -> Result<()> {
    // Use best-effort discovery so uninstall still works with a broken workspace.
    let (binaries, warning) = resolve_install_binaries_best_effort(ctx.paths);
    if let Some(message) = warning {
        log_line(
            ctx,
            format!(
                "Warning: binary discovery failed; using fallback list ({})",
                message
            ),
        );
    }

    for binary in binaries {
        let path = ctx.paths.bin_dir.join(binary);
        if path.exists() {
            fs::remove_file(&path).with_context(|| "failed to remove binary")?;
            log_line(ctx, format!("Removed binary {}", format_with_home(&path)));
        } else {
            log_line(
                ctx,
                format!("Binary not found at {}", format_with_home(&path)),
            );
        }
    }

    Ok(())
}

fn resolve_release_dir(ctx: &mut ActionContext) -> Result<PathBuf> {
    // Cargo metadata is the only reliable place to ask for the active target dir
    let target_dir = resolve_target_directory(ctx.paths).with_context(|| {
        format!(
            "failed to resolve cargo target directory for {}",
            format_with_home(&ctx.paths.repo_root)
        )
    })?;
    Ok(target_dir.join("release"))
}

fn copy_binary(ctx: &mut ActionContext, source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        return Err(anyhow!(
            "missing build artifact: {}",
            format_with_home(source)
        ));
    }

    let source_display = format_with_home(source);
    let destination_display = format_with_home(destination);
    // Copy to a temporary file in the target directory to keep updates atomic.
    let temp_name = format!(
        "{}.tmp-{}",
        destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        std::process::id()
    );
    let temp_path = destination.with_file_name(temp_name);

    if temp_path.exists() {
        // Clear stale temp files from previous interrupted installs.
        fs::remove_file(&temp_path).with_context(|| "failed to remove stale temp file")?;
    }

    fs::copy(source, &temp_path).map_err(|err| {
        anyhow!(
            "failed to stage {} -> {}: {}",
            source_display,
            format_with_home(&temp_path),
            err
        )
    })?;

    // On Linux, rename is atomic and replaces the destination when it exists.
    // Avoid pre-removal to prevent a window where the binary is missing.
    if let Err(err) = fs::rename(&temp_path, destination) {
        let _ = fs::remove_file(&temp_path);
        return Err(anyhow!(
            "failed to install {} -> {}: {}",
            source_display,
            destination_display,
            err
        ));
    }
    log_line(
        ctx,
        format!(
            "Installed {} -> {}",
            source.file_name().unwrap_or_default().to_string_lossy(),
            format_with_home(destination)
        ),
    );
    Ok(())
}

fn format_exec_start(paths: &InstallPaths) -> String {
    let path = paths.bin_dir.join("unixnotis-daemon");
    let rendered = format_with_home(&path);
    if let Some(tail) = rendered.strip_prefix("$HOME") {
        format!("%h{}", tail)
    } else {
        path.display().to_string()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceUnitWrite {
    CreatedOrUpdated,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceStartMode {
    EnableAndStart,
    StartOnly,
}

fn render_service_unit(paths: &InstallPaths) -> String {
    let exec_start = format_exec_start(paths);
    [
        "[Unit]".to_string(),
        "Description=UnixNotis Notification Daemon".to_string(),
        "After=graphical-session.target".to_string(),
        "Wants=graphical-session.target".to_string(),
        "".to_string(),
        "[Service]".to_string(),
        "Type=simple".to_string(),
        format!("ExecStart={}", exec_start),
        "Restart=on-failure".to_string(),
        "RestartSec=1".to_string(),
        "".to_string(),
        "[Install]".to_string(),
        "WantedBy=default.target".to_string(),
        "".to_string(),
    ]
    .join("\n")
}

fn write_service_unit(ctx: &mut ActionContext) -> Result<ServiceUnitWrite> {
    // Render first so the compare and write paths always use the same bytes
    let unit_contents = render_service_unit(ctx.paths);
    let existed_before = ctx.paths.unit_path.exists();
    let write_outcome = match fs::read_to_string(&ctx.paths.unit_path) {
        Ok(existing) if existing == unit_contents => ServiceUnitWrite::Unchanged,
        Ok(_) | Err(_) => {
            write_atomic(&ctx.paths.unit_path, &unit_contents)
                .with_context(|| "failed to write systemd user unit")?;
            ServiceUnitWrite::CreatedOrUpdated
        }
    };
    // Reload only matters when systemd has new unit file contents to pick up
    let reload_required =
        matches!(write_outcome, ServiceUnitWrite::CreatedOrUpdated) || !existed_before;
    ctx.service_unit_reload_required
        .store(reload_required, Ordering::Release);
    Ok(write_outcome)
}

fn service_start_mode(ctx: &ActionContext) -> ServiceStartMode {
    // Cached install state keeps the reinstall branch stable within one run
    service_start_mode_from_enabled(ctx.install_state.as_ref().map(|state| state.unit_enabled()))
}

fn service_start_mode_from_enabled(unit_enabled: Option<bool>) -> ServiceStartMode {
    if unit_enabled == Some(true) {
        // Reinstalls do not need another `enable`, which would trigger a full
        // user-manager reload on some desktops even when the unit is unchanged.
        ServiceStartMode::StartOnly
    } else {
        ServiceStartMode::EnableAndStart
    }
}

fn run_service_start(ctx: &mut ActionContext) -> Result<()> {
    match service_start_mode(ctx) {
        ServiceStartMode::EnableAndStart => {
            // First install still needs the symlink creation done by `enable`
            log_line(ctx, "Enabling and starting unixnotis-daemon.service");
            let mut enable = Command::new("systemctl");
            enable.args(["--user", "enable", "--now", "unixnotis-daemon.service"]);
            run_command(
                ctx,
                "systemctl --user enable --now unixnotis-daemon.service",
                enable,
                None,
            )
        }
        ServiceStartMode::StartOnly => {
            // Reinstall can start directly because the unit is already enabled
            log_line(ctx, "Starting unixnotis-daemon.service");
            let mut start = Command::new("systemctl");
            start.args(["--user", "start", "unixnotis-daemon.service"]);
            run_command(
                ctx,
                "systemctl --user start unixnotis-daemon.service",
                start,
                None,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::detect::Detection;
    use crate::events::UiMessage;
    use crate::model::ActionMode;
    use crate::paths::InstallPaths;

    use super::{
        install_binaries, install_service, remove_binaries, service_start_mode_from_enabled,
        ActionContext, ServiceStartMode,
    };

    #[test]
    fn install_binaries_copies_all_managed_binaries_including_noticenterctl() {
        // A fake workspace keeps the test focused on install behavior
        let root = test_root("install-binaries");
        write_fake_workspace(
            &root,
            &[
                "unixnotis-daemon",
                "unixnotis-popups",
                "unixnotis-center",
                "noticenterctl",
            ],
        );
        let paths = test_paths(&root);

        for binary in [
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "noticenterctl",
        ] {
            let source = paths.repo_root.join("target").join("release").join(binary);
            fs::create_dir_all(source.parent().expect("release dir")).expect("make release dir");
            fs::write(&source, format!("binary:{binary}")).expect("write fake binary");
        }

        let detection = Detection {
            owner: None,
            daemons: Vec::new(),
        };
        let (tx, _rx) = mpsc::sync_channel::<UiMessage>(32);
        let mut ctx = ActionContext {
            detection: &detection,
            paths: &paths,
            install_state: None,
            log_tx: tx,
            action_mode: ActionMode::Install,
            restore_backup: None,
            service_unit_reload_required: Arc::new(AtomicBool::new(false)),
        };

        install_binaries(&mut ctx).expect("install should copy binaries");

        for binary in [
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "noticenterctl",
        ] {
            let installed = paths.bin_dir.join(binary);
            assert!(installed.exists(), "{binary} should be installed");
            assert_eq!(
                fs::read_to_string(&installed).expect("read installed binary"),
                format!("binary:{binary}")
            );
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn remove_binaries_removes_all_managed_binaries_including_noticenterctl() {
        // Uninstall should remove the same binaries that install manages
        let root = test_root("remove-binaries");
        write_fake_workspace(
            &root,
            &[
                "unixnotis-daemon",
                "unixnotis-popups",
                "unixnotis-center",
                "noticenterctl",
            ],
        );
        let paths = test_paths(&root);

        fs::create_dir_all(&paths.bin_dir).expect("make bin dir");
        for binary in [
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "noticenterctl",
        ] {
            fs::write(paths.bin_dir.join(binary), format!("installed:{binary}"))
                .expect("write installed binary");
        }

        let detection = Detection {
            owner: None,
            daemons: Vec::new(),
        };
        let (tx, _rx) = mpsc::sync_channel::<UiMessage>(32);
        let mut ctx = ActionContext {
            detection: &detection,
            paths: &paths,
            install_state: None,
            log_tx: tx,
            action_mode: ActionMode::Uninstall,
            restore_backup: None,
            service_unit_reload_required: Arc::new(AtomicBool::new(false)),
        };

        remove_binaries(&mut ctx).expect("remove should delete binaries");

        for binary in [
            "unixnotis-daemon",
            "unixnotis-popups",
            "unixnotis-center",
            "noticenterctl",
        ] {
            assert!(
                !paths.bin_dir.join(binary).exists(),
                "{binary} should be removed"
            );
        }

        let _ = fs::remove_dir_all(&root);
    }

    fn test_root(name: &str) -> std::path::PathBuf {
        // Unique temp roots keep tests from stepping on each other
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "unixnotis-installer-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn test_paths(root: &std::path::Path) -> InstallPaths {
        InstallPaths {
            repo_root: root.to_path_buf(),
            bin_dir: root.join("home").join(".local").join("bin"),
            unit_dir: root
                .join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
            unit_path: root
                .join("home")
                .join(".config")
                .join("systemd")
                .join("user")
                .join("unixnotis-daemon.service"),
        }
    }

    fn write_fake_workspace(root: &std::path::Path, binaries: &[&str]) {
        // cargo metadata only needs a valid virtual workspace to report target dir
        fs::create_dir_all(root).expect("make fake workspace");
        let quoted = binaries
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let cargo_toml = format!(
            "[workspace]\nmembers = []\n\n[workspace.metadata.unixnotis.installer]\nbinaries = [{quoted}]\n"
        );
        fs::write(root.join("Cargo.toml"), cargo_toml).expect("write fake Cargo.toml");
    }

    #[test]
    fn install_service_skips_rewrite_when_unit_is_already_current() {
        let root = test_root("install-service-unchanged");
        let paths = test_paths(&root);
        fs::create_dir_all(&paths.unit_dir).expect("make unit dir");
        let expected = super::render_service_unit(&paths);
        fs::write(&paths.unit_path, &expected).expect("write current unit");

        let detection = Detection {
            owner: None,
            daemons: Vec::new(),
        };
        let (tx, _rx) = mpsc::sync_channel::<UiMessage>(32);
        let reload_required = Arc::new(AtomicBool::new(true));
        let mut ctx = ActionContext {
            detection: &detection,
            paths: &paths,
            install_state: None,
            log_tx: tx,
            action_mode: ActionMode::Install,
            restore_backup: None,
            service_unit_reload_required: reload_required.clone(),
        };

        install_service(&mut ctx).expect("install service should succeed");

        assert_eq!(
            fs::read_to_string(&paths.unit_path).expect("read unit"),
            expected
        );
        assert!(!reload_required.load(Ordering::Acquire));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn install_service_marks_reload_when_unit_changes() {
        let root = test_root("install-service-changed");
        let paths = test_paths(&root);
        fs::create_dir_all(&paths.unit_dir).expect("make unit dir");
        fs::write(&paths.unit_path, "[Unit]\nDescription=old\n").expect("write old unit");

        let detection = Detection {
            owner: None,
            daemons: Vec::new(),
        };
        let (tx, _rx) = mpsc::sync_channel::<UiMessage>(32);
        let reload_required = Arc::new(AtomicBool::new(false));
        let mut ctx = ActionContext {
            detection: &detection,
            paths: &paths,
            install_state: None,
            log_tx: tx,
            action_mode: ActionMode::Install,
            restore_backup: None,
            service_unit_reload_required: reload_required.clone(),
        };

        install_service(&mut ctx).expect("install service should succeed");

        assert!(reload_required.load(Ordering::Acquire));
        assert_eq!(
            fs::read_to_string(&paths.unit_path).expect("read unit"),
            super::render_service_unit(&paths)
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn service_start_mode_uses_start_for_enabled_reinstalls() {
        assert_eq!(
            service_start_mode_from_enabled(Some(true)),
            ServiceStartMode::StartOnly
        );
        assert_eq!(
            service_start_mode_from_enabled(Some(false)),
            ServiceStartMode::EnableAndStart
        );
        assert_eq!(
            service_start_mode_from_enabled(None),
            ServiceStartMode::EnableAndStart
        );
    }
}
