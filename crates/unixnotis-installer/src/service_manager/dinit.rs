use std::fs;
use std::path::{Path, PathBuf};

use super::artifact::{ServiceArtifact, ServiceArtifactKind};
use super::command::CommandSpec;
use super::readiness::ReadinessIssue;

// Dinit service names are file names without the .service suffix used by systemd
pub const SERVICE_NAME: &str = "unixnotis-daemon";

pub fn artifact_label() -> &'static str {
    "dinit service"
}

pub fn manager_label() -> &'static str {
    "dinit user manager"
}

pub fn primary_artifact_path(artifact_root: &Path) -> PathBuf {
    artifact_root.join(SERVICE_NAME)
}

pub fn artifacts(artifact_root: &Path, bin_dir: &Path) -> Vec<ServiceArtifact> {
    let boot_dir = artifact_root.join("boot.d");
    // The boot.d link mirrors dinitctl enable without letting dinit mutate user config
    vec![
        ServiceArtifact {
            path: boot_dir.clone(),
            kind: ServiceArtifactKind::Directory,
            contents: None,
            mode: None,
        },
        ServiceArtifact::file(
            primary_artifact_path(artifact_root),
            render_service(bin_dir),
        ),
        ServiceArtifact {
            path: boot_dir.join(SERVICE_NAME),
            kind: ServiceArtifactKind::Symlink {
                target: PathBuf::from(format!("../{SERVICE_NAME}")),
            },
            contents: None,
            mode: None,
        },
    ]
}

pub fn availability_command() -> Option<CommandSpec> {
    Some(
        CommandSpec::new(
            "dinitctl --user --quiet list",
            "dinitctl",
            ["--user", "--quiet", "list"],
        )
        .quiet(),
    )
}

pub fn is_enabled_command() -> Option<CommandSpec> {
    // Enablement is represented by the installer-owned boot.d link
    None
}

pub fn is_active_command() -> Option<CommandSpec> {
    Some(CommandSpec::new(
        format!("dinitctl --user --quiet is-started {SERVICE_NAME}"),
        "dinitctl",
        ["--user", "--quiet", "is-started", SERVICE_NAME],
    ))
}

pub fn reload_after_artifact_change() -> Option<CommandSpec> {
    // Start loads the service on first install; reload is fragile for services not already loaded
    None
}

pub fn enable_now_command() -> Option<CommandSpec> {
    // The boot.d artifact owns persistence; start only handles the live session
    start_command()
}

pub fn start_command() -> Option<CommandSpec> {
    Some(CommandSpec::new(
        format!("dinitctl --user start {SERVICE_NAME}"),
        "dinitctl",
        ["--user", "start", SERVICE_NAME],
    ))
}

pub fn disable_now_command() -> Option<CommandSpec> {
    Some(stop_ignoring_unstarted())
}

pub fn stop_for_reinstall_command() -> Option<CommandSpec> {
    Some(stop_ignoring_unstarted())
}

pub fn hyprland_startup_commands(import_vars: &[&str]) -> Vec<String> {
    // Restart updates an already running service; start covers fresh login sessions
    vec![
        format!("dinitctl --user setenv {}", import_vars.join(" ")),
        format!("dinitctl --user restart --ignore-unstarted {SERVICE_NAME}"),
        format!("dinitctl --user start {SERVICE_NAME}"),
    ]
}

pub fn environment_sync_commands(import_vars: &[(&str, String)]) -> Vec<CommandSpec> {
    let mut args = vec!["--user".to_string(), "setenv".to_string()];
    // dinitctl can read values from its own environment, which keeps values out of argv
    args.extend(import_vars.iter().map(|(name, _value)| (*name).to_string()));
    let mut spec = CommandSpec::new("dinitctl --user setenv", "dinitctl", &args);
    for (name, value) in import_vars {
        // Explicit env overrides avoid leaking session paths through installer logs or ps output
        spec = spec.env(*name, value);
    }
    vec![spec]
}

pub fn enabled_by_artifacts(artifact_root: &Path) -> bool {
    let service_path = primary_artifact_path(artifact_root);
    let boot_dir = artifact_root.join("boot.d");
    let boot_link = boot_dir.join(SERVICE_NAME);
    let expected_target = PathBuf::from(format!("../{SERVICE_NAME}"));

    // The boot link is the persistent enablement state, so verify the link itself
    is_regular_file(&service_path)
        && is_directory(&boot_dir)
        && fs::read_link(&boot_link)
            .map(|target| target == expected_target)
            .unwrap_or(false)
}

pub fn readiness_issues(artifact_root: &Path) -> Vec<ReadinessIssue> {
    // Dinit's user boot service is user-owned; installers should warn, not rewrite it
    if boot_service_includes_boot_dir(&artifact_root.join("boot")) {
        return Vec::new();
    }
    vec![ReadinessIssue::warning(
        "dinit boot service does not appear to include waits-for.d: boot.d; UnixNotis can start now, but may not start automatically next login"
    )]
}

fn stop_ignoring_unstarted() -> CommandSpec {
    CommandSpec::new(
        format!("dinitctl --user stop --ignore-unstarted {SERVICE_NAME}"),
        "dinitctl",
        ["--user", "stop", "--ignore-unstarted", SERVICE_NAME],
    )
}

fn render_service(bin_dir: &Path) -> String {
    let command = escape_command_token(&bin_dir.join("unixnotis-daemon"));
    // Keep the first dinit service artifact minimal until backend behavior is proven
    [
        "type = process".to_string(),
        format!("command = {command}"),
        "restart = on-failure".to_string(),
        String::new(),
    ]
    .join("\n")
}

fn escape_command_token(path: &Path) -> String {
    let raw = path.display().to_string();
    // Bare tokens are easier to read, but only when dinit will not reinterpret them
    if raw
        .chars()
        .all(|ch| !ch.is_whitespace() && !matches!(ch, '"' | '\\' | '#' | '$'))
    {
        return raw;
    }

    let mut escaped = String::with_capacity(raw.len() + 2);
    escaped.push('"');
    for ch in raw.chars() {
        match ch {
            '"' | '\\' => {
                // dinit treats backslash as an escape even inside quotes
                escaped.push('\\');
                escaped.push(ch);
            }
            // dinit expands $NAME in command values unless $ is doubled
            '$' => escaped.push_str("$$"),
            _ => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

fn boot_service_includes_boot_dir(path: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    contents.lines().any(|line| {
        // The readiness check is intentionally shallow; it should never rewrite user config
        let stripped = line
            .split_once('#')
            .map_or(line, |(before, _)| before)
            .trim();
        if stripped.is_empty() {
            return false;
        }
        let Some((raw_key, value)) = stripped.split_once([':', '=']) else {
            return false;
        };
        let key = raw_key.trim().trim_end_matches('+').trim();
        key.trim() == "waits-for.d"
            && value
                .split_whitespace()
                .any(|entry| entry.trim_matches('"') == "boot.d")
    })
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
