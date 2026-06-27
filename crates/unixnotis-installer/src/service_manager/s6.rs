use std::fs;
use std::path::{Path, PathBuf};

use unixnotis_core::program_in_path;

use super::artifact::{ServiceArtifact, ServiceArtifactKind, MANAGED_DIRECTORY_MARKER};
use super::command::CommandSpec;
use super::probe::ServiceProbe;
use super::readiness::ReadinessIssue;
use super::refresh::{S6DatabaseRefresh, ServiceArtifactRefresh};
use super::shell::{
    envdir_file_contents, envdir_sync_prelude, is_safe_env_name, render_envdir_shell_update,
    shell_quote, shell_quote_path,
};

pub const SERVICE_NAME: &str = "unixnotis-daemon";
const RUN_SCRIPT: &str = "run";
const TYPE_FILE: &str = "type";
const ENV_DIR: &str = "env";
const DEFAULT_BUNDLE: &str = "default";
const BUNDLE_TYPE: &str = "bundle\n";
const SAFE_RUN_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";

pub fn artifact_label() -> &'static str {
    "s6 user service source"
}

pub fn manager_label() -> &'static str {
    "s6-rc user database"
}

pub fn primary_artifact_path(artifact_root: &Path) -> PathBuf {
    service_dir(artifact_root)
}

pub fn artifacts(artifact_root: &Path, bin_dir: &Path) -> Vec<ServiceArtifact> {
    let service_dir = service_dir(artifact_root);
    let default_member = default_bundle_member(artifact_root);
    vec![
        ServiceArtifact {
            path: service_dir.clone(),
            kind: ServiceArtifactKind::ManagedDirectory,
            contents: None,
            mode: None,
        },
        ServiceArtifact {
            path: service_dir.join(TYPE_FILE),
            kind: ServiceArtifactKind::File,
            // s6-rc source directories declare daemon services as longruns
            contents: Some("longrun\n".to_string()),
            mode: Some(0o644),
        },
        ServiceArtifact {
            path: service_dir.join(RUN_SCRIPT),
            kind: ServiceArtifactKind::ExecutableFile,
            contents: Some(render_run_script(bin_dir)),
            mode: Some(0o755),
        },
        ServiceArtifact {
            path: default_bundle_type_file(artifact_root),
            kind: ServiceArtifactKind::SharedFile {
                created_marker: Some(default_bundle_type_marker(artifact_root)),
            },
            // The default bundle can be initialized safely only when missing or already valid
            contents: Some(BUNDLE_TYPE.to_string()),
            mode: Some(0o644),
        },
        ServiceArtifact {
            path: default_member,
            kind: ServiceArtifactKind::File,
            // Membership files are empty; the file name is the dependency edge
            contents: Some(String::new()),
            mode: Some(0o644),
        },
    ]
}

pub fn availability_command() -> Option<CommandSpec> {
    // s6 readiness needs several tools and paths, so readiness_issues owns validation
    None
}

pub fn is_enabled_command() -> Option<CommandSpec> {
    // Enablement is source-backed through the default bundle membership file
    None
}

pub fn enabled_by_artifacts(artifact_root: &Path) -> bool {
    let service = service_dir(artifact_root);
    is_directory(&service)
        && is_regular_file(&service.join(MANAGED_DIRECTORY_MARKER))
        && is_regular_file(&service.join(TYPE_FILE))
        && is_regular_file(&service.join(RUN_SCRIPT))
        && is_matching_file(&default_bundle_type_file(artifact_root), BUNDLE_TYPE)
        && is_regular_file(&default_bundle_member(artifact_root))
}

pub fn active_probe(live_dir: &Path) -> Option<ServiceProbe> {
    let service = live_service_dir(live_dir).display().to_string();
    // s6-svstat -o up is machine-readable and avoids parsing human status text
    let command = CommandSpec::new(
        format!("s6-svstat -o up {service}"),
        "s6-svstat",
        ["-o".to_string(), "up".to_string(), service],
    );
    Some(ServiceProbe::stdout(command, status_output_is_running))
}

pub fn refresh_after_artifact_change(
    artifact_root: &Path,
    live_dir: &Path,
) -> Option<ServiceArtifactRefresh> {
    // s6 source changes must be compiled into a database before s6-rc can see them
    Some(ServiceArtifactRefresh::S6Database(S6DatabaseRefresh::new(
        artifact_root.to_path_buf(),
        live_dir.to_path_buf(),
    )))
}

pub fn enable_now_command(live_dir: &Path) -> Option<CommandSpec> {
    start_command(live_dir)
}

pub fn start_command(live_dir: &Path) -> Option<CommandSpec> {
    Some(s6_rc_change_command(live_dir, "-u"))
}

pub fn disable_now_command(live_dir: &Path) -> Option<CommandSpec> {
    Some(s6_rc_change_command(live_dir, "-d"))
}

pub fn stop_for_reinstall_command(live_dir: &Path) -> Option<CommandSpec> {
    disable_now_command(live_dir)
}

pub fn hyprland_startup_commands(
    artifact_root: &Path,
    live_dir: &Path,
    import_vars: &[&str],
) -> Vec<String> {
    let env_dir = service_dir(artifact_root).join(ENV_DIR);
    let live_service = live_service_dir(live_dir);
    // Hyprland uses one exec-once line, so every shell step must be fail-closed
    let mut steps = envdir_sync_prelude(&env_dir);
    for var in import_vars
        .iter()
        .copied()
        .filter(|name| is_s6_envdir_name(name))
    {
        // Missing session vars intentionally become empty envdir files
        steps.push(render_envdir_shell_update(var));
    }
    steps.push(format!(
        "s6-rc -l {} -u change {} || exit 1",
        shell_quote_path(live_dir),
        shell_quote(SERVICE_NAME)
    ));
    steps.push(format!(
        "s6-svc -r {} || :",
        shell_quote_path(&live_service)
    ));
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
        if !is_s6_envdir_name(name) {
            return None;
        }
        let value = import_vars
            .iter()
            .find_map(|(candidate, value)| (*candidate == *name).then_some(value.as_str()));
        Some(ServiceArtifact {
            path: env_dir.join(name),
            kind: ServiceArtifactKind::File,
            // Empty files make s6-envdir remove stale variables from the service environment
            contents: Some(envdir_file_contents(value)),
            mode: Some(0o600),
        })
    }));
    artifacts
}

pub fn readiness_issues(artifact_root: &Path, live_dir: &Path) -> Vec<ReadinessIssue> {
    let mut issues = Vec::new();
    for program in [
        "s6-rc-compile",
        "s6-rc-update",
        "s6-rc",
        "s6-envdir",
        "s6-svstat",
    ] {
        // Missing tools would fail after artifact writes, so treat them as hard blockers
        if !program_in_path(program) {
            issues.push(ReadinessIssue::error(format!(
                "{program} not found in PATH; s6 backend cannot fully run"
            )));
        }
    }
    let default_type = default_bundle_type_file(artifact_root);
    match fs::read_to_string(&default_type) {
        Ok(contents) if contents == BUNDLE_TYPE => {}
        Ok(_) => {
            // An existing default bundle with different contents is user-owned state
            issues.push(ReadinessIssue::error(
                "s6 default bundle type file exists but is not 'bundle'; refusing to overwrite user service layout",
            ));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // Missing default bundle is safe to initialize because no existing file is replaced
            issues.push(ReadinessIssue::warning(
                "s6 default bundle is missing; installer will initialize a local default bundle",
            ));
        }
        Err(err) => issues.push(ReadinessIssue::error(format!(
            "failed to inspect s6 default bundle type file: {err}"
        ))),
    }
    if !is_directory(&artifact_root.join("sv")) {
        // The source root can be created on first install, but this hint explains the layout
        issues.push(ReadinessIssue::warning(
            "s6 source directory is missing; installer will create the local source tree",
        ));
    }
    if !is_directory(live_dir) {
        // Control commands need a live s6-rc tree before they can start or stop the service
        issues.push(ReadinessIssue::error(
            format!(
                "s6 live directory {} is missing; start local s6 supervision or set UNIXNOTIS_S6RC_LIVE_DIR before controlling UnixNotis",
                live_dir.display()
            ),
        ));
    }
    issues
}

fn render_run_script(bin_dir: &Path) -> String {
    // s6-supervise runs ./run from the service directory, so ./env is stable
    [
        "#!/bin/sh".to_string(),
        format!("PATH={}; export PATH", shell_quote(SAFE_RUN_PATH)),
        format!(
            "exec s6-envdir ./{} {}",
            ENV_DIR,
            shell_quote_path(&bin_dir.join(SERVICE_NAME))
        ),
        String::new(),
    ]
    .join("\n")
}

fn s6_rc_change_command(live_dir: &Path, direction: &'static str) -> CommandSpec {
    let live = live_dir.display().to_string();
    CommandSpec::new(
        format!("s6-rc -l {live} {direction} change {SERVICE_NAME}"),
        "s6-rc",
        [
            "-l".to_string(),
            live,
            direction.to_string(),
            "change".to_string(),
            SERVICE_NAME.to_string(),
        ],
    )
}

fn service_dir(artifact_root: &Path) -> PathBuf {
    artifact_root.join("sv").join(SERVICE_NAME)
}

fn default_bundle_dir(artifact_root: &Path) -> PathBuf {
    artifact_root.join("sv").join(DEFAULT_BUNDLE)
}

fn default_bundle_type_file(artifact_root: &Path) -> PathBuf {
    default_bundle_dir(artifact_root).join(TYPE_FILE)
}

fn default_bundle_type_marker(artifact_root: &Path) -> PathBuf {
    default_bundle_dir(artifact_root).join(".unixnotis-created-type")
}

fn default_bundle_member(artifact_root: &Path) -> PathBuf {
    default_bundle_dir(artifact_root)
        .join("contents.d")
        .join(SERVICE_NAME)
}

fn live_service_dir(live_dir: &Path) -> PathBuf {
    live_dir.join("servicedirs").join(SERVICE_NAME)
}

fn is_s6_envdir_name(name: &str) -> bool {
    // The run script sets PATH before s6-envdir, so session PATH is not imported
    name != "PATH" && is_safe_env_name(name)
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn is_matching_file(path: &Path, expected: &str) -> bool {
    is_regular_file(path)
        && fs::read_to_string(path)
            .map(|contents| contents == expected)
            .unwrap_or(false)
}

fn is_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn status_output_is_running(stdout: &str) -> bool {
    stdout.trim() == "true"
}
