use std::fs;
use std::path::{Path, PathBuf};

use unixnotis_core::program_in_path;

use super::artifact::{ServiceArtifact, ServiceArtifactKind, MANAGED_DIRECTORY_MARKER};
use super::command::CommandSpec;
use super::probe::ServiceProbe;

// Runit service directories use the service name directly under the supervision root
pub const SERVICE_NAME: &str = "unixnotis-daemon";
const RUN_SCRIPT: &str = "run";
const ENV_DIR: &str = "env";
const DOWN_FILE: &str = "down";
const SAFE_RUN_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";

pub fn artifact_label() -> &'static str {
    "runit service directory"
}

pub fn manager_label() -> &'static str {
    "runit user supervisor"
}

pub fn primary_artifact_path(artifact_root: &Path) -> PathBuf {
    service_dir(artifact_root)
}

pub fn artifacts(artifact_root: &Path, bin_dir: &Path) -> Vec<ServiceArtifact> {
    let service_dir = service_dir(artifact_root);
    // Directory comes first so later file artifacts can be written without parent races
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

pub fn availability_command() -> Option<CommandSpec> {
    // `sv -V` checks the control binary without requiring the service to exist yet
    Some(CommandSpec::new("sv -V", "sv", ["-V"]).quiet())
}

pub fn is_enabled_command() -> Option<CommandSpec> {
    // Enablement is the presence of the service directory under the watched root
    None
}

pub fn enabled_by_artifacts(artifact_root: &Path) -> bool {
    let service = service_dir(artifact_root);
    // A down file means runsv should not start the service automatically
    is_directory(&service)
        && is_regular_file(&service.join(MANAGED_DIRECTORY_MARKER))
        && is_regular_file(&service.join(RUN_SCRIPT))
        && path_is_missing(&service.join(DOWN_FILE))
}

pub fn active_probe(artifact_root: &Path) -> Option<ServiceProbe> {
    let service = service_dir_arg(artifact_root);
    let command = CommandSpec::new(
        format!("sv status {service}"),
        "sv",
        ["status".to_string(), service],
    );
    Some(ServiceProbe::stdout(command, status_output_is_running))
}

pub fn reload_after_artifact_change() -> Option<CommandSpec> {
    // Runit notices service-directory files through runsv; stop/start owns refresh behavior
    None
}

pub fn enable_now_command(artifact_root: &Path) -> Option<CommandSpec> {
    start_command(artifact_root)
}

pub fn start_command(artifact_root: &Path) -> Option<CommandSpec> {
    Some(sv_command("start", artifact_root))
}

pub fn disable_now_command(artifact_root: &Path) -> Option<CommandSpec> {
    Some(sv_command("stop", artifact_root))
}

pub fn stop_for_reinstall_command(artifact_root: &Path) -> Option<CommandSpec> {
    Some(sv_command("stop", artifact_root))
}

pub fn hyprland_startup_commands(artifact_root: &Path, import_vars: &[&str]) -> Vec<String> {
    let service = service_dir(artifact_root);
    let env_dir = service.join(ENV_DIR);
    // Hyprland needs one line, so join shell steps with semicolons instead of newlines
    let mut steps = vec![
        "umask 077".to_string(),
        format!("envdir={}", shell_quote_path(&env_dir)),
        "mkdir -p \"$envdir\" || exit".to_string(),
    ];
    for var in import_vars
        .iter()
        .copied()
        .filter(|name| is_runit_envdir_name(name))
    {
        // mktemp avoids following a preexisting envdir symlink; mv replaces the final path itself
        steps.push(render_envdir_shell_update(var));
    }
    steps.push(format!(
        "sv restart {} || sv start {}",
        shell_quote_path(&service),
        shell_quote_path(&service)
    ));
    // Values are read from the live session at runtime, never embedded in config text
    let script = steps.join("; ");
    vec![format!("sh -lc {}", shell_quote(&script))]
}

pub fn environment_sync_commands() -> Vec<CommandSpec> {
    Vec::new()
}

pub fn environment_sync_artifacts(
    artifact_root: &Path,
    import_var_names: &[&str],
    import_vars: &[(&str, String)],
) -> Vec<ServiceArtifact> {
    let env_dir = service_dir(artifact_root).join(ENV_DIR);
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
    pre_start_artifacts(artifact_root)
}

pub fn pre_start_artifacts_to_write(artifact_root: &Path) -> Vec<ServiceArtifact> {
    pre_start_artifacts(artifact_root)
}

pub fn readiness_warnings() -> Vec<String> {
    if program_in_path("chpst") {
        Vec::new()
    } else {
        vec!["chpst not found in PATH; runit service script cannot start UnixNotis".to_string()]
    }
}

fn pre_start_artifacts(artifact_root: &Path) -> Vec<ServiceArtifact> {
    vec![ServiceArtifact {
        path: service_dir(artifact_root).join(DOWN_FILE),
        kind: ServiceArtifactKind::File,
        // runsv will not start ./run while this file exists
        contents: Some(String::new()),
        mode: Some(0o600),
    }]
}

fn render_run_script(bin_dir: &Path) -> String {
    // runsv enters the service directory before executing ./run, so ./env is stable
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

fn envdir_value(value: &str) -> String {
    // chpst only reads the first line, so keep that same behavior before writing
    value
        .split(['\0', '\n'])
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '\t'])
        .to_string()
}

fn envdir_file_contents(value: Option<&str>) -> String {
    value.map_or_else(String::new, |value| format!("{}\n", envdir_value(value)))
}

fn render_envdir_shell_update(name: &str) -> String {
    [
        format!("tmp=$(mktemp \"$envdir/.{name}.XXXXXX\") || exit"),
        format!("printenv {name} > \"$tmp\" || : > \"$tmp\""),
        "chmod 600 \"$tmp\" || { rm -f \"$tmp\"; exit 1; }".to_string(),
        format!("mv -f \"$tmp\" \"$envdir/{name}\" || {{ rm -f \"$tmp\"; exit 1; }}"),
    ]
    .join("; ")
}

fn is_safe_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_runit_envdir_name(name: &str) -> bool {
    // The run script sets PATH before chpst, so session PATH is not imported into envdir
    name != "PATH" && is_safe_env_name(name)
}

fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}

fn shell_quote(raw: &str) -> String {
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

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn is_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn path_is_missing(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|_| false)
        .unwrap_or_else(|err| err.kind() == std::io::ErrorKind::NotFound)
}

fn status_output_is_running(stdout: &str) -> bool {
    stdout.trim_start().starts_with("run:")
}
