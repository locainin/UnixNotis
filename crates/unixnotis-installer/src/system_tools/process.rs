//! Bounded execution for trusted installer probes

use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use rustix::process::{kill_process_group, Pid, Signal};
use wait_timeout::ChildExt;

#[derive(Debug)]
pub struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

struct CapturedStream {
    bytes: Vec<u8>,
    truncated: bool,
}

struct BoundedCapture {
    bytes: Vec<u8>,
    max_bytes: usize,
    truncated: bool,
}

impl Write for BoundedCapture {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let retained = self
            .max_bytes
            .saturating_sub(self.bytes.len())
            .min(buffer.len());
        self.bytes.extend_from_slice(&buffer[..retained]);
        self.truncated |= retained != buffer.len();
        // Report the full write so excess data is drained without being retained
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn output_bounded(
    command: &mut Command,
    timeout: Duration,
    max_stream_bytes: usize,
) -> io::Result<BoundedOutput> {
    let deadline = probe_deadline(timeout)?;
    // A private process group lets timeout cleanup include helper grandchildren
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("probe stdout pipe was not captured"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("probe stderr pipe was not captured"))?;
    let stdout_reader = spawn_bounded_reader(stdout, max_stream_bytes);
    let stderr_reader = spawn_bounded_reader(stderr, max_stream_bytes);

    let status = wait_for_probe(&mut child, deadline)?;

    let stdout = receive_reader(&stdout_reader, deadline, "stdout")?;
    let stderr = receive_reader(&stderr_reader, deadline, "stderr")?;
    Ok(BoundedOutput {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
    })
}

fn probe_deadline(timeout: Duration) -> io::Result<Instant> {
    if timeout.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "probe deadline elapsed before process start",
        ));
    }
    Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| io::Error::other("probe deadline exceeded the monotonic clock"))
}

fn wait_for_probe(child: &mut Child, deadline: Instant) -> io::Result<ExitStatus> {
    match child.wait_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Some(status)) => {
            // A probe may exit after leaving a helper that still owns the output pipes
            let process_group = Pid::from_child(child);
            let _group_kill = kill_process_group(process_group, Signal::KILL);
            Ok(status)
        }
        Ok(None) => {
            // Group kill prevents a helper child from retaining captured pipes after timeout
            let process_group = Pid::from_child(child);
            let _group_kill = kill_process_group(process_group, Signal::KILL);
            let _direct_kill = child.kill();
            let _reap = child.wait();
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "probe process exceeded its deadline",
            ))
        }
        Err(error) => {
            let process_group = Pid::from_child(child);
            let _group_kill = kill_process_group(process_group, Signal::KILL);
            let _direct_kill = child.kill();
            let _reap = child.wait();
            Err(error)
        }
    }
}

fn spawn_bounded_reader(
    mut stream: impl Read + Send + 'static,
    max_stream_bytes: usize,
) -> Receiver<io::Result<CapturedStream>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut capture = BoundedCapture {
            bytes: Vec::with_capacity(max_stream_bytes.min(8 * 1024)),
            max_bytes: max_stream_bytes,
            truncated: false,
        };
        let result = io::copy(&mut stream, &mut capture).map(|_copied| CapturedStream {
            bytes: capture.bytes,
            truncated: capture.truncated,
        });
        let _sent = sender.send(result);
    });
    receiver
}

fn receive_reader(
    reader: &Receiver<io::Result<CapturedStream>>,
    deadline: Instant,
    stream_name: &str,
) -> io::Result<CapturedStream> {
    reader
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => io::Error::new(
                io::ErrorKind::TimedOut,
                format!("probe {stream_name} reader exceeded its deadline"),
            ),
            mpsc::RecvTimeoutError::Disconnected => {
                io::Error::other(format!("probe {stream_name} reader stopped unexpectedly"))
            }
        })?
}
