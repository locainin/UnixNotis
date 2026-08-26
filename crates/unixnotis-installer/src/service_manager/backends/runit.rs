use std::fs;
use std::path::{Path, PathBuf};

use crate::system_tools;

use super::super::contract::{
    envdir_file_contents, is_safe_env_name, shell_quote, shell_quote_path, CommandSpec,
    ReadinessIssue, ServiceArtifact, ServiceArtifactKind, ServiceProbe, ServiceProbeOutput,
    ServiceProbeState, MANAGED_DIRECTORY_MARKER,
};

// Runit service directories use the service name directly under the supervision root
pub const SERVICE_NAME: &str = "unixnotis-daemon";
const RUN_SCRIPT: &str = "run";
const ENV_DIR: &str = "env";
const DOWN_FILE: &str = "down";
const SAFE_RUN_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";

pub const fn artifact_label() -> &'static str {
    "runit service directory"
}

pub const fn manager_label() -> &'static str {
    "runit user supervisor"
}

pub fn primary_artifact_path(artifact_root: &Path) -> PathBuf {
    service_dir(artifact_root)
}

pub fn artifacts(artifact_root: &Path, bin_dir: &Path) -> Vec<ServiceArtifact> {
    let service_dir = service_dir(artifact_root);
    // Steady artifacts describe the installed service after the temporary down gate is removed
    vec![
        ServiceArtifact {
            path: service_dir.clone(),
            kind: ServiceArtifactKind::ManagedDirectory,
            contents: None,
            mode: None,
        },
        ServiceArtifact {
            path: service_dir.join(RUN_SCRIPT),
            kind: ServiceArtifactKind::ExecutableFile,
            contents: Some(render_run_script(bin_dir)),
            mode: Some(0o755),
        },
    ]
}

pub fn install_artifacts(artifact_root: &Path, bin_dir: &Path) -> Vec<ServiceArtifact> {
    let service_dir = service_dir(artifact_root);
    // Directory comes first so later file artifacts can be written without parent races
    // The down gate must come before ./run so runsv cannot start before envdir sync
    vec![
        ServiceArtifact {
            path: service_dir.clone(),
            kind: ServiceArtifactKind::ManagedDirectory,
            contents: None,
            mode: None,
        },
        start_gate_artifact(artifact_root),
        ServiceArtifact {
            path: service_dir.join(RUN_SCRIPT),
            kind: ServiceArtifactKind::ExecutableFile,
            contents: Some(render_run_script(bin_dir)),
            mode: Some(0o755),
        },
    ]
}

pub const fn is_enabled_command() -> Option<CommandSpec> {
    // Enablement is the presence of the service directory under the watched root
    None
}

pub fn enabled_by_artifacts(artifact_root: &Path) -> bool {
    let service = service_dir(artifact_root);
    // A down file means runsv should not start the service automatically
    // The managed marker prevents adopting a foreign service directory by accident
    is_directory(&service)
        && is_regular_file(&service.join(MANAGED_DIRECTORY_MARKER))
        && is_regular_file(&service.join(RUN_SCRIPT))
        && path_is_missing(&service.join(DOWN_FILE))
}

pub fn active_probe(artifact_root: &Path) -> ServiceProbe {
    let service = service_dir_arg(artifact_root);
    // sv check can succeed for a requested down state, so parse status text instead
    let command = CommandSpec::new(
        format!("sv status {service}"),
        "sv",
        ["status".to_string(), service],
    )
    // Runit diagnostics are stable English strings only under the C locale
    .env("LC_ALL", "C");
    ServiceProbe::new(command, interpret_active_state)
}

pub const fn reload_after_artifact_change() -> Option<CommandSpec> {
    // Runit notices service-directory files through runsv; stop/start owns refresh behavior
    None
}

pub fn enable_now_command(artifact_root: &Path) -> CommandSpec {
    start_command(artifact_root)
}

pub fn start_command(artifact_root: &Path) -> CommandSpec {
    sv_command("start", artifact_root)
}

pub fn disable_now_command(artifact_root: &Path) -> CommandSpec {
    sv_command("stop", artifact_root)
}

pub fn stop_for_reinstall_command(artifact_root: &Path) -> CommandSpec {
    sv_command("stop", artifact_root)
}

pub fn hyprland_startup_commands(_artifact_root: &Path, _import_vars: &[&str]) -> Vec<String> {
    vec!["noticenterctl doctor repair-session --service-manager runit".to_string()]
}

pub const fn environment_sync_commands() -> Vec<CommandSpec> {
    Vec::new()
}

pub fn environment_sync_artifacts(
    artifact_root: &Path,
    import_var_names: &[&str],
    import_vars: &[(&str, String)],
) -> Vec<ServiceArtifact> {
    let env_dir = service_dir(artifact_root).join(ENV_DIR);
    // Installer-time sync goes through hardened artifact writes instead of shell redirects
    let mut artifacts = vec![ServiceArtifact {
        path: env_dir.clone(),
        kind: ServiceArtifactKind::Directory,
        contents: None,
        mode: None,
    }];
    artifacts.extend(import_var_names.iter().filter_map(|name| {
        if !is_runit_envdir_name(name) {
            return None;
        }
        let value = import_vars
            .iter()
            .find_map(|(candidate, value)| (*candidate == *name).then_some(value.as_str()));
        Some(ServiceArtifact {
            path: env_dir.join(name),
            kind: ServiceArtifactKind::File,
            // Empty files make chpst remove stale variables from the service environment
            contents: Some(envdir_file_contents(value)),
            mode: Some(0o600),
        })
    }));
    artifacts
}

pub fn pre_start_artifacts_to_remove(artifact_root: &Path) -> Vec<ServiceArtifact> {
    vec![start_gate_artifact(artifact_root)]
}

pub const fn pre_start_artifacts_to_write(_artifact_root: &Path) -> Vec<ServiceArtifact> {
    // The gate is part of the ordered backend artifact list so it lands before ./run
    Vec::new()
}

pub fn readiness_issues() -> Vec<ReadinessIssue> {
    if system_tools::program_exists("chpst") {
        Vec::new()
    } else {
        vec![ReadinessIssue::error(
            "chpst not found in PATH; runit service script cannot start UnixNotis",
        )]
    }
}

fn start_gate_artifact(artifact_root: &Path) -> ServiceArtifact {
    ServiceArtifact {
        path: service_dir(artifact_root).join(DOWN_FILE),
        kind: ServiceArtifactKind::File,
        // runsv will not start ./run while this file exists
        contents: Some(String::new()),
        mode: Some(0o600),
    }
}

fn render_run_script(bin_dir: &Path) -> String {
    // runsv enters the service directory before executing ./run, so ./env is stable
    // PATH is fixed before chpst so synced session PATH cannot change command lookup
    [
        "#!/bin/sh".to_string(),
        format!("PATH={}; export PATH", shell_quote(SAFE_RUN_PATH)),
        format!(
            "exec chpst -e ./{} {}",
            ENV_DIR,
            shell_quote_path(&bin_dir.join("unixnotis-daemon"))
        ),
        String::new(),
    ]
    .join("\n")
}

fn sv_command(command: &'static str, artifact_root: &Path) -> CommandSpec {
    let service = service_dir_arg(artifact_root);
    // Pass the full service path so callers do not depend on SVDIR being exported
    CommandSpec::new(format!("sv {command} {service}"), "sv", [command, &service])
}

fn service_dir(artifact_root: &Path) -> PathBuf {
    artifact_root.join(SERVICE_NAME)
}

fn service_dir_arg(artifact_root: &Path) -> String {
    service_dir(artifact_root).display().to_string()
}

fn is_runit_envdir_name(name: &str) -> bool {
    // The run script sets PATH before chpst, so session PATH is not imported into envdir
    name != "PATH" && is_safe_env_name(name)
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}

fn is_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
}

fn path_is_missing(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map_or_else(|err| err.kind() == std::io::ErrorKind::NotFound, |_| false)
}

fn interpret_active_state(output: ServiceProbeOutput<'_>) -> ServiceProbeState {
    let stdout = output.stdout().trim();
    if output.status_success() {
        return if stdout.starts_with("run:") {
            ServiceProbeState::Active
        } else if stdout.starts_with("down:") {
            ServiceProbeState::Inactive
        } else {
            ServiceProbeState::Indeterminate
        };
    }

    // One-service probes return one only for this service-level failure class
    // Match only runit's documented absence diagnostics so timeouts stay blocking
    let service_is_absent = output.status_code() == Some(1)
        && output.stderr().trim().is_empty()
        && (stdout.ends_with(": runsv not running")
            || stdout
                .ends_with(": unable to change to service directory: No such file or directory"));
    if service_is_absent {
        ServiceProbeState::Absent
    } else {
        ServiceProbeState::Indeterminate
    }
}
