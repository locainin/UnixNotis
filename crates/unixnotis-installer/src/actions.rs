//! Installer actions for trial, install, and uninstall flows.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

use crate::detect::{DetectedDaemon, Detection};
use crate::events::{UiMessage, WorkerEvent};
use crate::model::{ActionMode, ActionStep, StepStatus};
use crate::paths::{format_with_home, InstallPaths};
use unixnotis_core::Config;

pub struct ActionContext<'a> {
    pub detection: &'a Detection,
    pub paths: &'a InstallPaths,
    pub install_state: Option<InstallState>,
    pub log_tx: Sender<UiMessage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepKind {
    InstallCheck,
    StopDaemon,
    Verify,
    Build,
    EnsureConfig,
    InstallBinaries,
    InstallService,
    EnableService,
    UninstallService,
    RemoveBinaries,
}

pub fn build_plan(mode: ActionMode, verify: bool) -> Vec<StepKind> {
    match mode {
        ActionMode::Test => Vec::new(),
        ActionMode::Install => {
            let mut steps = vec![StepKind::InstallCheck];
            if verify {
                steps.push(StepKind::Verify);
            }
            steps.extend([
                StepKind::Build,
                StepKind::EnsureConfig,
                StepKind::StopDaemon,
                StepKind::InstallBinaries,
                StepKind::InstallService,
                StepKind::EnableService,
            ]);
            steps
        }
        ActionMode::Uninstall => vec![StepKind::UninstallService, StepKind::RemoveBinaries],
    }
}

pub fn steps_from_plan(plan: &[StepKind]) -> Vec<ActionStep> {
    plan.iter()
        .map(|kind| ActionStep {
            name: step_label(*kind),
            status: StepStatus::Pending,
        })
        .collect()
}

pub fn run_step(step: StepKind, ctx: &mut ActionContext) -> Result<()> {
    match step {
        StepKind::InstallCheck => check_install_state_step(ctx),
        StepKind::StopDaemon => stop_active_daemon(ctx),
        StepKind::Verify => run_verify(ctx),
        StepKind::Build => run_build(ctx),
        StepKind::EnsureConfig => ensure_config(ctx),
        StepKind::InstallBinaries => install_binaries(ctx),
        StepKind::InstallService => install_service(ctx),
        StepKind::EnableService => enable_service(ctx),
        StepKind::UninstallService => uninstall_service(ctx),
        StepKind::RemoveBinaries => remove_binaries(ctx),
    }
}

pub fn step_label(kind: StepKind) -> &'static str {
    match kind {
        StepKind::InstallCheck => "Check existing install",
        StepKind::StopDaemon => "Stop existing daemon",
        StepKind::Verify => "Verify workspace",
        StepKind::Build => "Build release binaries",
        StepKind::EnsureConfig => "Ensure config files",
        StepKind::InstallBinaries => "Install binaries",
        StepKind::InstallService => "Install systemd unit",
        StepKind::EnableService => "Enable user service",
        StepKind::UninstallService => "Remove systemd unit",
        StepKind::RemoveBinaries => "Remove binaries",
    }
}

#[derive(Clone)]
pub struct BinaryState {
    name: &'static str,
    path: PathBuf,
    exists: bool,
}

#[derive(Clone)]
pub struct InstallState {
    binaries: Vec<BinaryState>,
    unit_exists: bool,
    unit_active: bool,
}

impl InstallState {
    pub fn is_fully_installed(&self) -> bool {
        self.binaries.iter().all(|binary| binary.exists) && self.unit_exists && self.unit_active
    }
}

pub fn check_install_state(paths: &InstallPaths) -> InstallState {
    let binaries = [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ]
    .into_iter()
    .map(|name| {
        let path = paths.bin_dir.join(name);
        BinaryState {
            name,
            exists: path.exists(),
            path,
        }
    })
    .collect::<Vec<_>>();

    let unit_exists = paths.unit_path.exists();
    let unit_active = Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", "unixnotis-daemon.service"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    InstallState {
        binaries,
        unit_exists,
        unit_active,
    }
}

fn check_install_state_step(ctx: &mut ActionContext) -> Result<()> {
    let state = ctx
        .install_state
        .clone()
        .unwrap_or_else(|| check_install_state(ctx.paths));

    log_line(ctx, "Install state:");
    for binary in &state.binaries {
        let status = if binary.exists { "present" } else { "missing" };
        log_line(
            ctx,
            format!(
                "- {}: {} ({})",
                binary.name,
                status,
                format_with_home(&binary.path)
            ),
        );
    }

    let unit_status = if state.unit_exists {
        "present"
    } else {
        "missing"
    };
    log_line(
        ctx,
        format!(
            "- systemd unit: {} ({})",
            unit_status,
            format_with_home(&ctx.paths.unit_path)
        ),
    );
    log_line(
        ctx,
        format!(
            "- service active: {}",
            if state.unit_active { "yes" } else { "no" }
        ),
    );

    if state.is_fully_installed() {
        log_line(ctx, "Already installed. No changes applied.");
    } else {
        log_line(ctx, "Install will continue and update missing items.");
    }

    Ok(())
}

fn stop_active_daemon(ctx: &mut ActionContext) -> Result<()> {
    let Some(owner) = ctx.detection.owner.as_ref() else {
        log_line(ctx, "No active notification daemon detected.");
        return Ok(());
    };

    let Some(comm) = owner.comm.as_deref() else {
        log_line(
            ctx,
            "Active owner detected, but command name is unavailable.",
        );
        return Ok(());
    };

    let known = ctx
        .detection
        .daemons
        .iter()
        .find(|daemon| daemon.name == comm);

    if let Some(daemon) = known {
        if daemon.systemd_active {
            let is_unixnotis = daemon.name == "unixnotis-daemon";
            log_line(ctx, format!("Stopping systemd unit {}", daemon.unit));
            let mut command = Command::new("systemctl");
            if is_unixnotis {
                command.args(["--user", "stop", daemon.unit.as_str()]);
            } else {
                command.args(["--user", "disable", "--now", daemon.unit.as_str()]);
            }
            let label = if is_unixnotis {
                format!("systemctl --user stop {}", daemon.unit)
            } else {
                format!("systemctl --user disable --now {}", daemon.unit)
            };
            run_command(ctx, &label, command, None)?;
            return Ok(());
        }

        if let Some(pid) = owner.pid {
            log_line(ctx, format!("Stopping {} (pid {})", daemon.name, pid));
            let status = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
                .context("failed to terminate notification daemon")?;
            if status.success() {
                wait_for_exit(ctx, pid)?;
                return Ok(());
            }
            return Err(anyhow!("failed to stop {}", daemon.name));
        }
    }

    log_line(
        ctx,
        format!("Detected owner '{}' is not managed by a known unit.", comm),
    );
    Ok(())
}

fn wait_for_exit(ctx: &mut ActionContext, pid: u32) -> Result<()> {
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    let poll = Duration::from_millis(100);

    while start.elapsed() < timeout {
        if !pid_alive(pid) {
            log_line(ctx, format!("Process {} stopped.", pid));
            return Ok(());
        }
        thread::sleep(poll);
    }

    Err(anyhow!("process {} did not exit after 5s", pid))
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_verify(ctx: &mut ActionContext) -> Result<()> {
    log_line(ctx, "Running cargo check/test/clippy");
    let mut check = Command::new("cargo");
    check.arg("check").env("RUSTFLAGS", "-D warnings");
    run_command(ctx, "cargo check", check, Some(&ctx.paths.repo_root))?;
    let mut test = Command::new("cargo");
    test.arg("test").env("RUSTFLAGS", "-D warnings");
    run_command(ctx, "cargo test", test, Some(&ctx.paths.repo_root))?;
    let mut clippy = Command::new("cargo");
    clippy.args([
        "clippy",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
        "-W",
        "clippy::perf",
    ]);
    run_command(ctx, "cargo clippy", clippy, Some(&ctx.paths.repo_root))?;
    Ok(())
}

fn run_build(ctx: &mut ActionContext) -> Result<()> {
    log_line(ctx, "Building release binaries");
    let mut build = Command::new("cargo");
    build.args([
        "build",
        "--release",
        "-p",
        "unixnotis-daemon",
        "-p",
        "unixnotis-popups",
        "-p",
        "unixnotis-center",
        "-p",
        "noticenterctl",
    ]);
    run_command(
        ctx,
        "cargo build --release",
        build,
        Some(&ctx.paths.repo_root),
    )?;
    Ok(())
}

fn ensure_config(ctx: &mut ActionContext) -> Result<()> {
    let config = Config::default();
    let config_dir = Config::default_config_dir().map_err(|err| anyhow!(err.to_string()))?;
    let config_path = Config::default_config_path().map_err(|err| anyhow!(err.to_string()))?;

    log_line(
        ctx,
        format!("Config directory: {}", format_with_home(&config_dir)),
    );

    if config_path.exists() {
        log_line(
            ctx,
            format!("Config file present: {}", format_with_home(&config_path)),
        );
    } else {
        log_line(
            ctx,
            format!("Config file missing: {}", format_with_home(&config_path)),
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

fn install_binaries(ctx: &mut ActionContext) -> Result<()> {
    let binaries = [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ];

    fs::create_dir_all(&ctx.paths.bin_dir).with_context(|| "failed to create bin directory")?;

    for binary in binaries {
        let source = ctx.paths.release_dir.join(binary);
        let destination = ctx.paths.bin_dir.join(binary);
        copy_binary(ctx, &source, &destination)?;
    }

    Ok(())
}

fn install_service(ctx: &mut ActionContext) -> Result<()> {
    fs::create_dir_all(&ctx.paths.unit_dir)
        .with_context(|| "failed to create systemd user directory")?;

    let exec_start = format_exec_start(ctx.paths);
    let unit_contents = [
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
    .join("\n");

    fs::write(&ctx.paths.unit_path, unit_contents)
        .with_context(|| "failed to write systemd user unit")?;

    log_line(
        ctx,
        format!(
            "Installed systemd unit to {}",
            format_with_home(&ctx.paths.unit_path)
        ),
    );

    Ok(())
}

fn enable_service(ctx: &mut ActionContext) -> Result<()> {
    let mut daemon_reload = Command::new("systemctl");
    daemon_reload.args(["--user", "daemon-reload"]);
    run_command(ctx, "systemctl --user daemon-reload", daemon_reload, None)?;
    let mut enable = Command::new("systemctl");
    enable.args(["--user", "enable", "--now", "unixnotis-daemon.service"]);
    run_command(
        ctx,
        "systemctl --user enable --now unixnotis-daemon.service",
        enable,
        None,
    )?;
    Ok(())
}

fn uninstall_service(ctx: &mut ActionContext) -> Result<()> {
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

    Ok(())
}

fn remove_binaries(ctx: &mut ActionContext) -> Result<()> {
    let binaries = [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ];

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

fn copy_binary(ctx: &mut ActionContext, source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        return Err(anyhow!(
            "missing build artifact: {}",
            format_with_home(source)
        ));
    }

    let source_display = format_with_home(source);
    let destination_display = format_with_home(destination);
    fs::copy(source, destination).map_err(|err| {
        anyhow!(
            "failed to install {} -> {}: {}",
            source_display,
            destination_display,
            err
        )
    })?;
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

fn run_command(
    ctx: &mut ActionContext,
    label: &str,
    mut command: Command,
    cwd: Option<&PathBuf>,
) -> Result<()> {
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("command failed to start: {}", label))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let log_tx = ctx.log_tx.clone();

    let stdout_handle = stdout.map(|stream| {
        let tx = log_tx.clone();
        thread::spawn(move || read_stream(stream, tx))
    });

    let stderr_handle = stderr.map(|stream| {
        let tx = log_tx.clone();
        thread::spawn(move || read_stream(stream, tx))
    });

    let status = child
        .wait()
        .with_context(|| format!("command failed to run: {}", label))?;

    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("command failed: {}", label))
    }
}

fn log_line(ctx: &mut ActionContext, line: impl Into<String>) {
    let _ = ctx
        .log_tx
        .send(UiMessage::Worker(WorkerEvent::LogLine(line.into())));
}

fn sanitize_log_line(line: &str) -> String {
    line.replace('\r', "")
}

fn read_stream(stream: impl std::io::Read, tx: Sender<UiMessage>) {
    let reader = BufReader::new(stream);
    for line in reader.lines().map_while(Result::ok) {
        let _ = tx.send(UiMessage::Worker(WorkerEvent::LogLine(sanitize_log_line(
            &line,
        ))));
    }
}

pub fn summarize_owner(owner: &Option<crate::detect::OwnerInfo>) -> String {
    match owner {
        Some(info) => {
            let name = info.comm.as_deref().unwrap_or("unknown");
            let pid = info
                .pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            format!("{} (pid {})", name, pid)
        }
        None => "none detected".to_string(),
    }
}

pub fn format_daemon_status(daemon: &DetectedDaemon) -> String {
    let mut status = Vec::new();
    if daemon.is_owner {
        status.push("dbus-owner".to_string());
    }
    if daemon.systemd_active {
        status.push("systemd-active".to_string());
    }
    if !daemon.running_pids.is_empty() {
        let ids = daemon
            .running_pids
            .iter()
            .map(|pid| pid.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        status.push(format!("pid {}", ids));
    }
    if status.is_empty() {
        status.push("not running".to_string());
    }
    status.join(", ")
}
