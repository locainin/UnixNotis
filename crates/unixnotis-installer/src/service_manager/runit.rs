use std::path::{Path, PathBuf};

use super::artifact::{ServiceArtifact, ServiceArtifactKind};
use super::command::CommandSpec;

// Runit service directories use the service name directly under the supervision root
pub const SERVICE_NAME: &str = "unixnotis-daemon";
const RUN_SCRIPT: &str = "run";
const ENV_DIR: &str = "env";

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
    vec![
        ServiceArtifact {
            path: service_dir.clone(),
            kind: ServiceArtifactKind::Directory,
            contents: None,
            mode: None,
        },
        ServiceArtifact {
            path: service_dir.join(ENV_DIR),
            kind: ServiceArtifactKind::Directory,
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
    service.is_dir() && service.join(RUN_SCRIPT).is_file() && !service.join("down").exists()
}

pub fn is_active_command(artifact_root: &Path) -> Option<CommandSpec> {
    let service = service_dir_arg(artifact_root);
    Some(CommandSpec::new(
        format!("sv check {service}"),
        "sv",
        ["check".to_string(), service],
    ))
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
    let mut steps = vec![format!("mkdir -p {} || exit", shell_quote_path(&env_dir))];
    for var in import_vars {
        // `printenv` writes an empty file when the variable is missing, which makes chpst unset it
        steps.push(format!(
            "printenv {var} > {} || :",
            shell_quote_path(&env_dir.join(var))
        ));
    }
    steps.push(format!(
        "sv restart {} || sv start {}",
        shell_quote_path(&service),
        shell_quote_path(&service)
    ));
    let script = steps.join("; ");
    vec![format!("sh -lc {}", shell_quote(&script))]
}

pub fn environment_sync_commands() -> Vec<CommandSpec> {
    Vec::new()
}

pub fn environment_sync_artifacts(
    artifact_root: &Path,
    import_vars: &[(&str, String)],
) -> Vec<ServiceArtifact> {
    let env_dir = service_dir(artifact_root).join(ENV_DIR);
    let mut artifacts = vec![ServiceArtifact {
        path: env_dir.clone(),
        kind: ServiceArtifactKind::Directory,
        contents: None,
        mode: None,
    }];
    artifacts.extend(import_vars.iter().map(|(name, value)| ServiceArtifact {
        path: env_dir.join(name),
        kind: ServiceArtifactKind::File,
        // chpst reads only the first line and strips trailing spaces and tabs
        contents: Some(format!("{}\n", envdir_value(value))),
        mode: Some(0o600),
    }));
    artifacts
}

fn render_run_script(bin_dir: &Path) -> String {
    [
        "#!/bin/sh".to_string(),
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
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}
