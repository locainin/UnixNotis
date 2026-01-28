//! Command execution helpers shared by widget workers.
//!
//! Consolidates process spawning, timeout handling, and pipe draining so the
//! queueing logic can stay focused on backpressure and scheduling.

use std::io::{self, Read};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use tokio::io::AsyncReadExt;
use tokio::process::Command as TokioCommand;
use tokio::runtime::Runtime;
use tracing::warn;
use wait_timeout::ChildExt;

use super::command_parse::parse_simple_command;

pub(super) fn build_command_runtime() -> Option<Runtime> {
    // A lightweight runtime enables async pipe reads without spawning extra threads.
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
    let mut child = spawn_capture_command_async(cmd)?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Drain stdout/stderr on the runtime to avoid per-command reader threads.
    let stdout_handle = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut stdout) = stdout {
            let _ = stdout.read_to_end(&mut buf).await;
        }
        buf
    });
    let stderr_handle = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_end(&mut buf).await;
        }
        buf
    });

    let status = if timeout.is_zero() {
        child.wait().await?
    } else {
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(status) => status?,
            Err(_) => {
                // Kill on timeout to keep worker throughput predictable.
                kill_child_process(&mut child).await;
                stdout_handle.abort();
                stderr_handle.abort();
                return Err(io::Error::new(io::ErrorKind::TimedOut, "command timed out"));
            }
        }
    };

    let stdout = stdout_handle.await.unwrap_or_default();
    let stderr = stderr_handle.await.unwrap_or_default();
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
    if timeout.is_zero() {
        return child.wait_with_output();
    }

    let stdout_handle = match child.stdout.take() {
        Some(stdout) => spawn_reader(stdout),
        None => std::thread::spawn(Vec::new),
    };
    let stderr_handle = match child.stderr.take() {
        Some(stderr) => spawn_reader(stderr),
        None => std::thread::spawn(Vec::new),
    };

    let pid = child.id() as i32;
    // Block on the OS wait call with a timeout instead of polling in a tight loop.
    // This keeps idle CPU near zero while preserving deterministic timeouts.
    let status = match child.wait_timeout(timeout)? {
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
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn spawn_reader<R: Read + Send + 'static>(mut reader: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        buf
    })
}

pub(super) fn spawn_capture_command(cmd: &str) -> io::Result<Child> {
    let mut command = build_command(cmd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.spawn()
}

pub(super) fn build_command(cmd: &str) -> Command {
    if let Some((program, args)) = parse_simple_command(cmd) {
        let mut command = Command::new(program);
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
        let mut command = TokioCommand::new(program);
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
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn configure_command_tokio(command: &mut TokioCommand) {
    // Use a dedicated process group so timeouts can kill the whole subtree.
    command.stdin(Stdio::null());
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

pub(in crate::ui::widgets) fn kill_process_group(pid: i32) {
    if pid <= 0 {
        return;
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
}
