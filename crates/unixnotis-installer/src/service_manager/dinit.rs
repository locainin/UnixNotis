use std::fs;
use std::path::{Path, PathBuf};

use super::artifact::{ServiceArtifact, ServiceArtifactKind};
use super::command::CommandSpec;

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
    vec![
        format!("dinitctl --user setenv {}", import_vars.join(" ")),
        format!("dinitctl --user restart --ignore-unstarted {SERVICE_NAME}"),
        format!("dinitctl --user start {SERVICE_NAME}"),
    ]
}

pub fn environment_sync_commands(import_vars: &[(&str, String)]) -> Vec<CommandSpec> {
    let mut args = vec!["--user".to_string(), "setenv".to_string()];
    args.extend(
        import_vars
            .iter()
            .map(|(name, value)| format!("{name}={value}")),
    );
    vec![CommandSpec::new(
        "dinitctl --user setenv",
        "dinitctl",
        &args,
    )]
}

pub fn enabled_by_artifacts(artifact_root: &Path) -> bool {
    let service_path = primary_artifact_path(artifact_root);
    let boot_dir = artifact_root.join("boot.d");
    let boot_link = boot_dir.join(SERVICE_NAME);
    let expected_target = PathBuf::from(format!("../{SERVICE_NAME}"));

    // The boot link is the persistent enablement state, so verify the link itself
    service_path.is_file()
        && boot_dir.is_dir()
        && fs::read_link(&boot_link)
            .map(|target| target == expected_target)
            .unwrap_or(false)
}

pub fn readiness_warnings(artifact_root: &Path) -> Vec<String> {
    if boot_service_includes_boot_dir(&artifact_root.join("boot")) {
        return Vec::new();
    }
    vec![
        "dinit boot service does not appear to include waits-for.d: boot.d; UnixNotis can start now, but may not start automatically next login"
            .to_string(),
    ]
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
                escaped.push('\\');
                escaped.push(ch);
            }
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
        let stripped = line
            .split_once('#')
            .map_or(line, |(before, _)| before)
            .trim();
        if stripped.is_empty() {
            return false;
        }
        let Some((key, value)) = stripped.split_once([':', '=']) else {
            return false;
        };
        key.trim() == "waits-for.d"
            && value
                .split_whitespace()
                .any(|entry| entry.trim_matches('"') == "boot.d")
    })
}
