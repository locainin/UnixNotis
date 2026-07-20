//! Timeout execution for blocking and Tokio workers

use std::io;
use std::process::Output;
use std::time::Duration;
use unixnotis_core::CommandSpec;

use tokio::runtime::Runtime;
use tracing::warn;
use wait_timeout::ChildExt;

use super::builder::{spawn_capture_command, spawn_capture_command_async};
use super::output::{
    join_async_reader, join_blocking_reader, read_to_end_limited_async, spawn_reader,
};
use super::process::{kill_child_process, kill_process_group};

pub(in crate::ui::widgets::utils::command) fn build_command_runtime() -> Option<Runtime> {
    // A current-thread runtime keeps frequent widget probes lightweight
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

pub(in crate::ui::widgets::utils::command) fn run_command_with_timeout(
    cmd: &CommandSpec,
    timeout: Duration,
    runtime: Option<&Runtime>,
) -> Result<Output, io::Error> {
    // A supplied runtime enables nonblocking pipe draining
    if let Some(runtime) = runtime {
        return run_command_with_timeout_async(cmd, timeout, runtime);
    }
    run_command_with_timeout_blocking(cmd, timeout)
}

fn run_command_with_timeout_async(
    cmd: &CommandSpec,
    timeout: Duration,
    runtime: &Runtime,
) -> Result<Output, io::Error> {
    runtime.block_on(async { run_command_with_timeout_inner(cmd, timeout).await })
}

async fn run_command_with_timeout_inner(
    cmd: &CommandSpec,
    timeout: Duration,
) -> io::Result<Output> {
    let mut child = spawn_capture_command_async(cmd)?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Both streams drain concurrently so either pipe can fill safely
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
        // Zero keeps the existing no-timeout contract
        child.wait().await
    } else if let Ok(status) = tokio::time::timeout(timeout, child.wait()).await {
        status
    } else {
        kill_child_process(&mut child).await;
        stdout_handle.abort();
        stderr_handle.abort();
        return Err(io::Error::new(io::ErrorKind::TimedOut, "command timed out"));
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

fn run_command_with_timeout_blocking(cmd: &CommandSpec, timeout: Duration) -> io::Result<Output> {
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
    // wait_timeout blocks efficiently while retaining a deterministic deadline
    let status = if timeout.is_zero() {
        child.wait()?
    } else if let Some(status) = child.wait_timeout(timeout)? {
        status
    } else {
        kill_process_group(pid);
        let _ = child.kill();
        let _ = child.wait();
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();
        return Err(io::Error::new(io::ErrorKind::TimedOut, "command timed out"));
    };

    let stdout = join_blocking_reader(stdout_handle, "stdout")?;
    let stderr = join_blocking_reader(stderr_handle, "stderr")?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(test)]
#[path = "tests/runner.rs"]
mod tests;
