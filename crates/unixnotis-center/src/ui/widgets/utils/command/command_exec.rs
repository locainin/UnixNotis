//! Command execution helpers shared by widget workers.
//!
//! Consolidates process spawning, timeout handling, and pipe draining so the
//! queueing logic can stay focused on backpressure and scheduling.

use std::io::{self, Read};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

#[cfg(unix)]
use rustix::process::{Pid, Signal};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command as TokioCommand;
use tokio::runtime::Runtime;
use tracing::warn;
use unixnotis_core::Config;
use wait_timeout::ChildExt;

use super::command_parse::parse_simple_command;

// Bound captured command output to prevent large stdout/stderr from ballooning memory.
const MAX_CAPTURE_BYTES: usize = 1024 * 1024;
// Missing target used when a command tries to leave the config dir
const BLOCKED_OUTSIDE_ROOT_PROGRAM: &str = ".unixnotis-blocked-command-path";

pub(super) fn build_command_runtime() -> Option<Runtime> {
    // A lightweight runtime enables async pipe reads without spawning extra threads,
    // which keeps per-command overhead low for frequent widget refreshes.
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| {
            warn!(
                ?err,
                "failed to build command runtime, falling back to blocking I/O"
            );
            err
        })
        .ok()
}

pub(super) fn run_command_with_timeout(
    cmd: &str,
    timeout: Duration,
    runtime: Option<&Runtime>,
) -> Result<Output, io::Error> {
    // Prefer async I/O when a runtime is available; fall back to blocking in worst case.
    if let Some(runtime) = runtime {
        return run_command_with_timeout_async(cmd, timeout, runtime);
    }
    run_command_with_timeout_blocking(cmd, timeout)
}

fn run_command_with_timeout_async(
    cmd: &str,
    timeout: Duration,
    runtime: &Runtime,
) -> Result<Output, io::Error> {
    runtime.block_on(async { run_command_with_timeout_inner(cmd, timeout).await })
}

async fn run_command_with_timeout_inner(cmd: &str, timeout: Duration) -> Result<Output, io::Error> {
    // Spawn the command with piped stdout/stderr so both streams can be drained.
    let mut child = spawn_capture_command_async(cmd)?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Drain stdout/stderr on the runtime to avoid per-command reader threads.
    let stdout_handle = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            return read_to_end_limited_async(stdout).await;
        }
        Ok(Vec::new())
    });
    let stderr_handle = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            return read_to_end_limited_async(stderr).await;
        }
        Ok(Vec::new())
    });

    let status_result = if timeout.is_zero() {
        // Zero timeout indicates "no timeout" rather than "immediate timeout."
        child.wait().await
    } else {
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(status) => status,
            Err(_) => {
                // Kill on timeout to keep worker throughput predictable.
                kill_child_process(&mut child).await;
                stdout_handle.abort();
                stderr_handle.abort();
                return Err(io::Error::new(io::ErrorKind::TimedOut, "command timed out"));
            }
        }
    };
    let status = match status_result {
        Ok(status) => status,
        Err(err) => {
            stdout_handle.abort();
            stderr_handle.abort();
            return Err(err);
        }
    };

    let stdout = join_async_reader(stdout_handle, "stdout").await?;
    let stderr = join_async_reader(stderr_handle, "stderr").await?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

async fn kill_child_process(child: &mut tokio::process::Child) {
    // Best-effort kill; ensures the child is reaped even if signal delivery races.
    if let Some(pid) = child.id() {
        kill_process_group(pid as i32);
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn run_command_with_timeout_blocking(cmd: &str, timeout: Duration) -> Result<Output, io::Error> {
    let mut child = spawn_capture_command(cmd)?;

    let stdout_handle = match child.stdout.take() {
        Some(stdout) => spawn_reader(stdout),
        None => std::thread::spawn(|| Ok(Vec::new())),
    };
    let stderr_handle = match child.stderr.take() {
        Some(stderr) => spawn_reader(stderr),
        None => std::thread::spawn(|| Ok(Vec::new())),
    };

    let pid = child.id() as i32;
    // Block on the OS wait call with a timeout instead of polling in a tight loop.
    // This keeps idle CPU near zero while preserving deterministic timeouts.
    let status = if timeout.is_zero() {
        // Consistent with async path: 0 means no timeout.
        child.wait()?
    } else {
        match child.wait_timeout(timeout)? {
            Some(status) => status,
            None => {
                // Kill on timeout to keep worker throughput predictable.
                kill_process_group(pid);
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return Err(io::Error::new(io::ErrorKind::TimedOut, "command timed out"));
            }
        }
    };

    let stdout = join_blocking_reader(stdout_handle, "stdout")?;
    let stderr = join_blocking_reader(stderr_handle, "stderr")?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn spawn_reader<R: Read + Send + 'static>(
    reader: R,
) -> std::thread::JoinHandle<io::Result<Vec<u8>>> {
    // Dedicated reader thread avoids blocking command worker while draining pipes.
    std::thread::spawn(move || read_to_end_limited(reader))
}

fn join_blocking_reader(
    handle: std::thread::JoinHandle<io::Result<Vec<u8>>>,
    stream: &str,
) -> io::Result<Vec<u8>> {
    match handle.join() {
        Ok(result) => result.map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to read command {stream} stream: {err}"),
            )
        }),
        Err(_) => Err(io::Error::other(format!(
            "command {stream} reader thread panicked"
        ))),
    }
}

async fn join_async_reader(
    handle: tokio::task::JoinHandle<io::Result<Vec<u8>>>,
    stream: &str,
) -> io::Result<Vec<u8>> {
    match handle.await {
        Ok(result) => result.map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to read command {stream} stream: {err}"),
            )
        }),
        Err(err) => Err(io::Error::other(format!(
            "command {stream} reader task failed: {err}"
        ))),
    }
}

fn read_to_end_limited<R: Read>(reader: R) -> io::Result<Vec<u8>> {
    // Read at most limit + 1 so overflow can be detected without extra passes
    let mut limited = reader.take((MAX_CAPTURE_BYTES as u64) + 1);
    let mut buf = Vec::new();
    limited.read_to_end(&mut buf)?;
    if buf.len() > MAX_CAPTURE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("command output exceeded {MAX_CAPTURE_BYTES} bytes"),
        ));
    }
    Ok(buf)
}

async fn read_to_end_limited_async<R: AsyncRead + Unpin>(reader: R) -> io::Result<Vec<u8>> {
    // Mirror blocking implementation so size limits are identical in both paths
    let mut limited = reader.take((MAX_CAPTURE_BYTES as u64) + 1);
    let mut buf = Vec::new();
    limited.read_to_end(&mut buf).await?;
    if buf.len() > MAX_CAPTURE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("command output exceeded {MAX_CAPTURE_BYTES} bytes"),
        ));
    }
    Ok(buf)
}

pub(super) fn spawn_capture_command(cmd: &str) -> io::Result<Child> {
    let mut command = build_command(cmd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.spawn()
}

pub(super) fn build_command(cmd: &str) -> Command {
    if let Some((program, args)) = parse_simple_command(cmd) {
        // Simple commands avoid shell invocation for safety and performance.
        let mut command = Command::new(resolve_simple_program(&program));
        command.args(args);
        configure_command(&mut command);
        return command;
    }

    let mut command = Command::new("sh");
    // Non-login shell avoids profile sourcing on every widget refresh.
    command.arg("-c").arg(cmd);
    configure_command(&mut command);
    command
}

pub(super) fn spawn_capture_command_async(cmd: &str) -> io::Result<tokio::process::Child> {
    // Mirrors the blocking builder but returns a tokio child with piped output.
    let mut command = build_tokio_command(cmd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.spawn()
}

pub(super) fn build_tokio_command(cmd: &str) -> TokioCommand {
    if let Some((program, args)) = parse_simple_command(cmd) {
        // Tokio command mirrors the blocking path for consistent behavior.
        let mut command = TokioCommand::new(resolve_simple_program(&program));
        command.args(args);
        configure_command_tokio(&mut command);
        return command;
    }

    // Shell fallback keeps behavior consistent with previous implementation.
    let mut command = TokioCommand::new("sh");
    command.arg("-c").arg(cmd);
    configure_command_tokio(&mut command);
    command
}

fn configure_command(command: &mut Command) {
    command.stdin(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
}

fn configure_command_tokio(command: &mut TokioCommand) {
    // Use a dedicated process group so timeouts can kill the whole subtree.
    command.stdin(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
}

fn resolve_simple_program(program: &str) -> PathBuf {
    // Runtime lookup keeps shared presets working after export rewrites host-local paths
    resolve_simple_program_from_root(Config::default_config_dir().ok().as_deref(), program)
}

fn resolve_simple_program_from_root(config_dir: Option<&Path>, program: &str) -> PathBuf {
    let path = Path::new(program);
    if !looks_like_relative_path_program(program, path) {
        return path.to_path_buf();
    }

    // Preset imports rewrite bundled scripts to config-root-relative paths for portability
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
    // Bare names still use PATH lookup, while path-like names are rooted in the config dir
    !path.is_absolute()
        && (program == "."
            || program.starts_with("./")
            || program.starts_with("../")
            || program.contains('/'))
}

fn command_path_escapes_root(config_dir: &Path, rooted_path: &Path) -> bool {
    // Catch ../ escapes without touching disk
    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_candidate = normalize_lexical_path(rooted_path);
    !normalized_candidate.starts_with(&normalized_root)
}

fn normalize_lexical_path(path: &Path) -> PathBuf {
    // Keep path cleanup lexical only
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

pub(in crate::ui::widgets) fn kill_process_group(pid: i32) {
    if pid <= 0 {
        return;
    }
    #[cfg(unix)]
    {
        if let Some(pid) = Pid::from_raw(pid) {
            let _ = rustix::process::kill_process_group(pid, Signal::KILL);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor};
    use std::path::Path;

    use super::{read_to_end_limited, resolve_simple_program_from_root, MAX_CAPTURE_BYTES};

    #[test]
    fn read_to_end_limited_accepts_small_payloads() {
        let payload = b"ok".to_vec();
        let result = read_to_end_limited(Cursor::new(payload.clone())).expect("small payload");
        assert_eq!(result, payload);
    }

    #[test]
    fn read_to_end_limited_rejects_oversized_payloads() {
        let payload = vec![0u8; MAX_CAPTURE_BYTES + 1];
        let err = read_to_end_limited(Cursor::new(payload)).expect_err("oversized payload");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn resolve_simple_program_roots_relative_script_paths_in_config_dir() {
        let config_dir = Path::new("/tmp/demo/unixnotis");

        assert_eq!(
            resolve_simple_program_from_root(Some(config_dir), "scripts/demo-widget"),
            config_dir.join("scripts/demo-widget")
        );
    }

    #[test]
    fn resolve_simple_program_blocks_parent_traversal_paths() {
        let config_dir = Path::new("/tmp/demo/unixnotis");

        assert_eq!(
            resolve_simple_program_from_root(Some(config_dir), "../outside-script"),
            config_dir.join(".unixnotis-blocked-command-path")
        );
    }
}
