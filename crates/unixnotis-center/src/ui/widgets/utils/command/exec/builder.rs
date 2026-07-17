//! Blocking and asynchronous child process construction

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};

use tokio::process::Command as TokioCommand;
use tracing::warn;
use unixnotis_core::Config;

use super::super::command_parse::{parse_simple_command, ParsedCommand};

// Missing target used when a command tries to leave the config dir
const BLOCKED_OUTSIDE_ROOT_PROGRAM: &str = ".unixnotis-blocked-command-path";
// Keep warning memory bounded while still suppressing repeat shell-fallback spam
const SHELL_FALLBACK_CACHE_LIMIT: usize = 64;

static COMMAND_CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

pub(in crate::ui::widgets::utils::command) fn set_command_config_dir(config_dir: PathBuf) -> bool {
    // Widget commands are built after startup, so retain the active custom config root
    if COMMAND_CONFIG_DIR.get() == Some(&config_dir) {
        return true;
    }
    if COMMAND_CONFIG_DIR.set(config_dir).is_err() {
        warn!("widget command config directory was already initialized");
        return false;
    }
    true
}

pub(super) fn spawn_capture_command(cmd: &str) -> std::io::Result<Child> {
    let mut command = build_command(cmd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.spawn()
}

pub(in crate::ui::widgets::utils::command) fn build_command(cmd: &str) -> Command {
    if let Some(parsed) = parse_simple_command(cmd) {
        // Simple commands avoid shell invocation for safety and performance
        let mut command = Command::new(resolve_simple_program(&parsed.program));
        apply_parsed_command_env(&mut command, &parsed);
        command.args(&parsed.args);
        configure_command(&mut command);
        return command;
    }

    let _ = log_shell_fallback_once(cmd);
    let mut command = Command::new("sh");
    // Non-login shell avoids profile sourcing on every widget refresh
    command.arg("-c").arg(cmd);
    configure_command(&mut command);
    command
}

pub(super) fn spawn_capture_command_async(cmd: &str) -> std::io::Result<tokio::process::Child> {
    // Mirrors the blocking builder but returns a Tokio child with piped output
    let mut command = build_tokio_command(cmd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.spawn()
}

fn build_tokio_command(cmd: &str) -> TokioCommand {
    if let Some(parsed) = parse_simple_command(cmd) {
        // Tokio command mirrors the blocking path for consistent behavior
        let mut command = TokioCommand::new(resolve_simple_program(&parsed.program));
        apply_parsed_command_env_tokio(&mut command, &parsed);
        command.args(&parsed.args);
        configure_command_tokio(&mut command);
        return command;
    }

    // Shell fallback keeps blocking and asynchronous behavior aligned
    let _ = log_shell_fallback_once(cmd);
    let mut command = TokioCommand::new("sh");
    command.arg("-c").arg(cmd);
    configure_command_tokio(&mut command);
    command
}

fn log_shell_fallback_once(cmd: &str) -> bool {
    let hash = shell_fallback_hash(cmd);
    let cache = shell_fallback_cache();
    let mut cache = match cache.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };

    if cache.contains(&hash) {
        return false;
    }
    if cache.len() >= SHELL_FALLBACK_CACHE_LIMIT {
        cache.remove(0);
    }
    cache.push(hash);
    drop(cache);

    // Shell-backed commands are expected, but preset review still needs a visible signal
    warn!(
        command = %unixnotis_core::util::log_snippet(cmd),
        command_hash = format_args!("{hash:016x}"),
        "widget command is using shell fallback"
    );
    true
}

fn shell_fallback_cache() -> &'static Mutex<Vec<u64>> {
    static CACHE: OnceLock<Mutex<Vec<u64>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

fn shell_fallback_hash(cmd: &str) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cmd.hash(&mut hasher);
    hasher.finish()
}

fn configure_command(command: &mut Command) {
    command.stdin(Stdio::null());
    if let Some(config_dir) = command_config_dir() {
        // Validation resolves relative command inputs against this same directory
        command.current_dir(config_dir);
    }
    #[cfg(unix)]
    command.process_group(0);
}

fn apply_parsed_command_env(command: &mut Command, parsed: &ParsedCommand) {
    // Only this child receives command-specific environment overrides
    for (name, value) in &parsed.env {
        command.env(name, value);
    }
}

fn configure_command_tokio(command: &mut TokioCommand) {
    command.stdin(Stdio::null());
    if let Some(config_dir) = command_config_dir() {
        // Tokio and blocking children share the loader and relative-path base
        command.current_dir(config_dir);
    }
    #[cfg(unix)]
    command.process_group(0);
}

fn apply_parsed_command_env_tokio(command: &mut TokioCommand, parsed: &ParsedCommand) {
    // Timeout strategy must not change the child's environment
    for (name, value) in &parsed.env {
        command.env(name, value);
    }
}

fn resolve_simple_program(program: &str) -> PathBuf {
    // Runtime lookup keeps exported config-relative scripts portable
    resolve_simple_program_from_root(command_config_dir().as_deref(), program)
}

pub(in crate::ui::widgets::utils::command) fn command_config_dir() -> Option<PathBuf> {
    if let Some(config_dir) = COMMAND_CONFIG_DIR.get() {
        return Some(config_dir.clone());
    }

    Config::default_config_dir().ok()
}

fn resolve_simple_program_from_root(config_dir: Option<&Path>, program: &str) -> PathBuf {
    let path = Path::new(program);
    if !looks_like_relative_path_program(program, path) {
        return path.to_path_buf();
    }

    // Preset imports rewrite bundled scripts to config-root-relative paths
    if let Some(config_dir) = config_dir {
        let rooted = config_dir.join(path);
        if command_path_escapes_root(config_dir, &rooted) {
            warn!(
                command = %program,
                root = %config_dir.display(),
                "blocked path-like command that escapes the UnixNotis config directory"
            );
            return config_dir.join(BLOCKED_OUTSIDE_ROOT_PROGRAM);
        }
        return rooted;
    }

    path.to_path_buf()
}

fn looks_like_relative_path_program(program: &str, path: &Path) -> bool {
    // Bare names still use PATH lookup, while path-like names use the config dir
    !path.is_absolute() && (program == "." || program.contains('/'))
}

fn command_path_escapes_root(config_dir: &Path, rooted_path: &Path) -> bool {
    // Catch parent traversal without requiring either path to exist
    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_candidate = normalize_lexical_path(rooted_path);
    !normalized_candidate.starts_with(&normalized_root)
}

fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}
