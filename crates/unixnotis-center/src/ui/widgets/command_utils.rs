//! Command execution, budgeting, and watch helpers for widgets.

use std::io::{self, Read};
use std::process::{Child, Command, Output, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crossbeam_channel as channel;
use tracing::warn;
use unixnotis_core::util;
use unixnotis_core::PanelDebugLevel;

use crate::debug;

const COMMAND_WORKERS: usize = 2;
const FAST_TIMEOUT_MS: u64 = 350;
const SLOW_TIMEOUT_MS: u64 = 800;
const ACTION_TIMEOUT_MS: u64 = 1200;
const SLOW_JITTER_MS: u64 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui::widgets) enum CommandKind {
    Fast,
    Slow,
    Action,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::ui::widgets) struct CommandPlan {
    kind: CommandKind,
}

impl CommandPlan {
    fn timeout(self) -> Duration {
        match self.kind {
            CommandKind::Fast => Duration::from_millis(FAST_TIMEOUT_MS),
            CommandKind::Slow => Duration::from_millis(SLOW_TIMEOUT_MS),
            CommandKind::Action => Duration::from_millis(ACTION_TIMEOUT_MS),
        }
    }

    fn jitter(self) -> Duration {
        if self.kind != CommandKind::Slow || SLOW_JITTER_MS == 0 {
            return Duration::from_millis(0);
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        let jitter_ms = (nanos % (SLOW_JITTER_MS * 1_000_000)) / 1_000_000;
        Duration::from_millis(jitter_ms)
    }

    pub(in crate::ui::widgets) fn spawn_watch_command(&self, cmd: &str) -> io::Result<Child> {
        let mut command = build_command(cmd);
        command.stdout(Stdio::piped()).stderr(Stdio::null());
        command.spawn()
    }
}

pub(in crate::ui::widgets) fn resolve_command_plan(
    cmd: &str,
    default_kind: CommandKind,
) -> CommandPlan {
    let mut kind = default_kind;
    if default_kind != CommandKind::Action && is_probably_slow(cmd) {
        kind = CommandKind::Slow;
    }
    CommandPlan { kind }
}

pub(in crate::ui::widgets) fn run_command(cmd: &str) {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        warn!("command was empty");
        return;
    }
    debug::log(PanelDebugLevel::Verbose, || {
        let snippet = util::log_snippet(cmd);
        format!("enqueue action command: {snippet}")
    });
    enqueue_command(
        cmd.to_string(),
        resolve_command_plan(cmd, CommandKind::Action),
        None,
    );
}

pub(in crate::ui::widgets) fn run_command_capture_async(
    cmd: &str,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    let (tx, rx) = async_channel::bounded(1);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        let _ = tx.send_blocking(Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command was empty",
        )));
        return rx;
    }
    let plan = resolve_command_plan(cmd, CommandKind::Slow);
    debug::log(PanelDebugLevel::Verbose, || {
        let snippet = util::log_snippet(cmd);
        format!("enqueue slow command: {snippet}")
    });
    enqueue_command(cmd.to_string(), plan, Some(tx));
    rx
}

pub(in crate::ui::widgets) fn run_command_capture_status_async(
    cmd: &str,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    let (tx, rx) = async_channel::bounded(1);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        let _ = tx.send_blocking(Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command was empty",
        )));
        return rx;
    }
    let plan = resolve_command_plan(cmd, CommandKind::Fast);
    debug::log(PanelDebugLevel::Verbose, || {
        let snippet = util::log_snippet(cmd);
        format!("enqueue fast command: {snippet}")
    });
    enqueue_command(cmd.to_string(), plan, Some(tx));
    rx
}

struct CommandJob {
    cmd: String,
    plan: CommandPlan,
    respond: Option<async_channel::Sender<Result<Output, io::Error>>>,
}

struct CommandWorker {
    tx: channel::Sender<CommandJob>,
    inline_fallback: bool,
}

impl CommandWorker {
    fn global() -> &'static CommandWorker {
        static WORKER: OnceLock<CommandWorker> = OnceLock::new();
        WORKER.get_or_init(|| CommandWorker::new(COMMAND_WORKERS))
    }

    fn new(worker_count: usize) -> Self {
        let (tx, rx) = channel::unbounded();
        let mut spawned = 0usize;
        for idx in 0..worker_count.max(1) {
            let rx = rx.clone();
            match std::thread::Builder::new()
                .name(format!("unixnotis-command-worker-{idx}"))
                .spawn(move || run_worker(rx))
            {
                Ok(_) => spawned += 1,
                Err(err) => {
                    warn!(?err, "failed to spawn command worker thread");
                }
            }
        }
        if spawned == 0 {
            warn!("no command worker threads available; falling back to inline execution");
        }
        Self {
            tx,
            inline_fallback: spawned == 0,
        }
    }
}

fn enqueue_command(
    cmd: String,
    plan: CommandPlan,
    respond: Option<async_channel::Sender<Result<Output, io::Error>>>,
) {
    let job = CommandJob { cmd, plan, respond };
    let worker = CommandWorker::global();
    if worker.inline_fallback {
        handle_job(job);
        return;
    }
    if worker.tx.send(job).is_err() {
        warn!("command worker channel closed");
    }
}

fn run_worker(rx: channel::Receiver<CommandJob>) {
    for job in rx.iter() {
        handle_job(job);
    }
}

fn handle_job(job: CommandJob) {
    let cmd_snip = util::log_snippet(&job.cmd);
    debug::log(PanelDebugLevel::Verbose, || {
        format!("command start kind={:?} cmd={}", job.plan.kind, cmd_snip)
    });
    let started = Instant::now();
    let jitter = job.plan.jitter();
    if !jitter.is_zero() {
        std::thread::sleep(jitter);
    }
    let result = run_command_with_timeout(&job.cmd, job.plan.timeout());
    let elapsed_ms = started.elapsed().as_millis();
    if let Some(tx) = job.respond {
        let _ = tx.send_blocking(result);
        return;
    }
    match result {
        Ok(output) => {
            if !output.status.success() {
                warn!(command = %cmd_snip, "command returned non-zero status");
                debug::log(PanelDebugLevel::Warn, || {
                    format!(
                        "command failed kind={:?} status={:?} elapsed_ms={elapsed_ms}",
                        job.plan.kind,
                        output.status.code()
                    )
                });
            } else {
                debug::log(PanelDebugLevel::Verbose, || {
                    format!(
                        "command ok kind={:?} status={:?} elapsed_ms={elapsed_ms}",
                        job.plan.kind,
                        output.status.code()
                    )
                });
            }
        }
        Err(err) => {
            warn!(command = %cmd_snip, ?err, "command failed");
            debug::log(PanelDebugLevel::Warn, || {
                format!(
                    "command error kind={:?} elapsed_ms={elapsed_ms} err={err}",
                    job.plan.kind
                )
            });
        }
    }
}

fn run_command_with_timeout(cmd: &str, timeout: Duration) -> Result<Output, io::Error> {
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
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            let stdout = stdout_handle.join().unwrap_or_default();
            let stderr = stderr_handle.join().unwrap_or_default();
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }
        if started.elapsed() >= timeout {
            kill_process_group(pid);
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_handle.join();
            let _ = stderr_handle.join();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "command timed out"));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn spawn_reader<R: Read + Send + 'static>(mut reader: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        buf
    })
}

fn spawn_capture_command(cmd: &str) -> io::Result<Child> {
    let mut command = build_command(cmd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.spawn()
}

fn build_command(cmd: &str) -> Command {
    if let Some((program, args)) = parse_simple_command(cmd) {
        let mut command = Command::new(program);
        command.args(args);
        configure_command(&mut command);
        return command;
    }

    let mut command = Command::new("sh");
    command.arg("-lc").arg(cmd);
    configure_command(&mut command);
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

fn parse_simple_command(cmd: &str) -> Option<(String, Vec<String>)> {
    let cmd = cmd.trim();
    if cmd.is_empty() || !is_simple_command(cmd) {
        return None;
    }
    let mut parts = cmd.split_whitespace();
    let program = parts.next()?.to_string();
    let args = parts.map(str::to_string).collect();
    Some((program, args))
}

fn is_simple_command(cmd: &str) -> bool {
    const META: [char; 18] = [
        '|', '&', ';', '<', '>', '$', '`', '\\', '"', '\'', '(', ')', '{', '}', '[', ']', '*', '?',
    ];
    if cmd
        .chars()
        .any(|ch| META.contains(&ch) || ch == '~' || ch == '\n' || ch == '\r')
    {
        return false;
    }

    let first = cmd.split_whitespace().next().unwrap_or_default();
    if first.contains('=') && !first.starts_with('/') && !first.starts_with("./") {
        return false;
    }

    true
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

fn is_probably_slow(cmd: &str) -> bool {
    let lower = cmd.to_ascii_lowercase();
    let has_pipeline =
        lower.contains('|') || lower.contains("&&") || lower.contains("||") || lower.contains(';');
    if has_pipeline || lower.contains("sleep") {
        return true;
    }
    [
        "nmcli",
        "bluetoothctl",
        "rfkill",
        "udevadm",
        "upower",
        "playerctl",
        "pactl",
        "wpctl",
        "brightnessctl",
    ]
    .iter()
    .any(|token| lower.contains(token))
}
