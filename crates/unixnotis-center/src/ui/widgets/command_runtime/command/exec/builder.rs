//! Blocking and asynchronous child process construction

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};

use tokio::process::Command as TokioCommand;
use tracing::warn;
use unixnotis_core::{filesystem::ContainedPath, CommandSpec, Config};

// Missing target used when a command tries to leave the config dir
const BLOCKED_OUTSIDE_ROOT_PROGRAM: &str = ".unixnotis-blocked-command-path";
// Keep warning memory bounded while still suppressing repeat shell-fallback spam
const SHELL_FALLBACK_CACHE_LIMIT: usize = 64;

static COMMAND_CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

pub(in crate::ui::widgets::command_runtime::command) fn set_command_config_dir(
    config_dir: PathBuf,
) -> bool {
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

pub(super) fn spawn_capture_command(cmd: &CommandSpec) -> std::io::Result<Child> {
    let mut command = build_command(cmd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.spawn()
}

pub(in crate::ui::widgets::command_runtime::command) fn build_command(
    cmd: &CommandSpec,
) -> Command {
    let mut command = match cmd {
        CommandSpec::Direct { program, args, env } => {
            let mut command = Command::new(resolve_direct_program(program));
            command.args(args).envs(env);
            command
        }
        CommandSpec::Shell { script } => {
            let _ = log_shell_fallback_once(script);
            let mut command = Command::new("sh");
            // Non-login shell avoids profile sourcing on every widget refresh
            command.arg("-c").arg(script);
            command
        }
    };
    configure_command(&mut command);
    command
}

pub(super) fn spawn_capture_command_async(
    cmd: &CommandSpec,
) -> std::io::Result<tokio::process::Child> {
    // Mirrors the blocking builder but returns a Tokio child with piped output
    let mut command = build_tokio_command(cmd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.spawn()
}

fn build_tokio_command(cmd: &CommandSpec) -> TokioCommand {
    let mut command = match cmd {
        CommandSpec::Direct { program, args, env } => {
            let mut command = TokioCommand::new(resolve_direct_program(program));
            command.args(args).envs(env);
            command
        }
        CommandSpec::Shell { script } => {
            let _ = log_shell_fallback_once(script);
            let mut command = TokioCommand::new("sh");
            command.arg("-c").arg(script);
            command
        }
    };
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

fn configure_command_tokio(command: &mut TokioCommand) {
    command.stdin(Stdio::null());
    if let Some(config_dir) = command_config_dir() {
        // Tokio and blocking children share the loader and relative-path base
        command.current_dir(config_dir);
    }
    #[cfg(unix)]
    command.process_group(0);
}

fn resolve_direct_program(program: &Path) -> PathBuf {
    // Runtime lookup keeps exported config-relative scripts portable
    resolve_direct_program_from_root(command_config_dir().as_deref(), program)
}

pub(in crate::ui::widgets::command_runtime::command) fn command_config_dir() -> Option<PathBuf> {
    if let Some(config_dir) = COMMAND_CONFIG_DIR.get() {
        return Some(config_dir.clone());
    }

    Config::default_config_dir().ok()
}

fn resolve_direct_program_from_root(config_dir: Option<&Path>, program: &Path) -> PathBuf {
    if !looks_like_relative_path_program(program) {
        return program.to_path_buf();
    }

    // Preset imports rewrite bundled scripts to config-root-relative paths
    if let Some(config_dir) = config_dir {
        let Ok(contained) = ContainedPath::resolve_relative(config_dir, program) else {
            warn!(
                command = %program.display(),
                root = %config_dir.display(),
                "blocked path-like command that escapes the UnixNotis config directory"
            );
            return config_dir.join(BLOCKED_OUTSIDE_ROOT_PROGRAM);
        };
        return contained.absolute();
    }

    program.to_path_buf()
}

fn looks_like_relative_path_program(program: &Path) -> bool {
    // Bare names still use PATH lookup, while path-like names use the config dir
    !program.is_absolute() && (program == Path::new(".") || program.components().count() > 1)
}

#[cfg(test)]
#[path = "tests/builder.rs"]
mod tests;
