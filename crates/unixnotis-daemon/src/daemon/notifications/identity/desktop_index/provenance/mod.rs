//! Immutable installation ownership used by desktop attribution

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use rustix::process::{kill_process_group, Pid, Signal};

use super::super::executable::executable_evidence_for_path;
use wait_timeout::ChildExt;

const MAX_OWNERSHIP_PATHS: usize = 16_384;
const MAX_COMMAND_ARGUMENT_BYTES: usize = 192 * 1024;
const MAX_COMMAND_PATHS: usize = 4_096;
const MAX_OWNERSHIP_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_PACKAGE_ID_BYTES: usize = 256;
const PACKAGE_QUERY_TIMEOUT: Duration = Duration::from_secs(1);
const PACKAGE_PIPE_DRAIN_TIMEOUT: Duration = Duration::from_millis(50);
const TRANSIENT_NEGATIVE_TTL: Duration = Duration::from_secs(30);
const NOT_OWNED_NEGATIVE_TTL: Duration = Duration::from_mins(5);
const MAX_RPM_QUERY_PATHS: usize = 4_096;
const MAX_RPM_QUERY_WORKERS: usize = 8;
const RPM_TOTAL_QUERY_TIMEOUT: Duration = Duration::from_secs(2);

/// System database that established package ownership
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub(in crate::daemon::notifications) enum PackageProvider {
    Pacman,
    Dpkg,
    Rpm,
}

#[derive(Debug, Clone)]
struct PackageProviderCommand {
    provider: PackageProvider,
    executable: PathBuf,
}

/// Installation source shared by protected desktop and executable files
#[derive(Debug, Clone, Default, Eq, Hash, PartialEq)]
pub(in crate::daemon::notifications) enum InstallProvenance {
    Package {
        provider: PackageProvider,
        package_id: String,
    },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "bundle ownership is part of the closed provenance model before a backend is available"
        )
    )]
    ImmutableBundle { bundle_id: String },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "portal ownership is retained as a separate authority domain"
        )
    )]
    Portal { app_id: String },
    #[default]
    Unknown,
}

impl InstallProvenance {
    pub(in crate::daemon::notifications::identity) fn same_application_source(
        &self,
        other: &Self,
    ) -> bool {
        match (self, other) {
            (
                Self::Package {
                    provider: left_provider,
                    package_id: left_id,
                },
                Self::Package {
                    provider: right_provider,
                    package_id: right_id,
                },
            ) => left_provider == right_provider && left_id == right_id,
            (
                Self::ImmutableBundle { bundle_id: left },
                Self::ImmutableBundle { bundle_id: right },
            )
            | (Self::Portal { app_id: left }, Self::Portal { app_id: right }) => left == right,
            _ => false,
        }
    }

    pub(in crate::daemon::notifications::identity) const fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum NegativeCause {
    NotOwned,
    Timeout,
    ProviderFailure,
    MalformedOutput,
    ProcessTermination,
}

#[derive(Debug, Clone)]
enum CachedProvenance {
    Known(InstallProvenance),
    Negative {
        retry_after: Instant,
        cause: NegativeCause,
    },
}

impl CachedProvenance {
    fn from_lookup(lookup: OwnershipLookup, now: Instant) -> Self {
        match lookup {
            OwnershipLookup::Known(provenance) => Self::Known(provenance),
            OwnershipLookup::Negative(cause) => Self::Negative {
                retry_after: now.checked_add(negative_ttl(cause)).unwrap_or(now),
                cause,
            },
        }
    }

    fn needs_refresh(&self, now: Instant) -> bool {
        let Self::Negative { retry_after, cause } = self else {
            return false;
        };
        // Keeping the cause live preserves the distinction used to select retry windows
        debug_assert!(
            !negative_ttl(*cause).is_zero(),
            "negative package-provenance results must remain retryable"
        );
        now >= *retry_after
    }

    fn provenance(&self) -> InstallProvenance {
        match self {
            Self::Known(provenance) => provenance.clone(),
            Self::Negative { .. } => InstallProvenance::Unknown,
        }
    }
}

const fn negative_ttl(cause: NegativeCause) -> Duration {
    match cause {
        NegativeCause::NotOwned => NOT_OWNED_NEGATIVE_TTL,
        NegativeCause::Timeout
        | NegativeCause::ProviderFailure
        | NegativeCause::MalformedOutput
        | NegativeCause::ProcessTermination => TRANSIENT_NEGATIVE_TTL,
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum OwnershipLookup {
    Known(InstallProvenance),
    Negative(NegativeCause),
}

#[derive(Debug, Default)]
pub(super) struct PackageOwnershipCache {
    provider: OnceLock<Option<PackageProviderCommand>>,
    entries: Mutex<HashMap<PathBuf, CachedProvenance>>,
}

impl PackageOwnershipCache {
    pub(super) fn resolve_many(
        &self,
        paths: impl IntoIterator<Item = PathBuf>,
    ) -> HashMap<PathBuf, InstallProvenance> {
        // Dedupe before taking the cache lock so repeated desktop aliases stay cheap
        let paths = paths
            .into_iter()
            .take(MAX_OWNERSHIP_PATHS)
            .collect::<HashSet<_>>();
        let now = Instant::now();
        let missing = self.entries.lock().map_or_else(
            |_| paths.iter().cloned().collect::<Vec<_>>(),
            |entries| {
                paths
                    .iter()
                    .filter(|path| {
                        entries
                            .get(*path)
                            .is_none_or(|entry| entry.needs_refresh(now))
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            },
        );

        if !missing.is_empty() {
            let resolved = self
                .provider
                .get_or_init(detect_package_provider)
                .as_ref()
                .map_or_else(
                    || {
                        missing
                            .iter()
                            .cloned()
                            .map(|path| {
                                (
                                    path,
                                    OwnershipLookup::Negative(NegativeCause::ProviderFailure),
                                )
                            })
                            .collect()
                    },
                    |provider| query_package_ownership(provider, &missing),
                );
            let resolved_at = Instant::now();
            if let Ok(mut entries) = self.entries.lock() {
                for path in missing {
                    let lookup = resolved
                        .get(&path)
                        .cloned()
                        .unwrap_or(OwnershipLookup::Negative(NegativeCause::ProviderFailure));
                    entries.insert(path, CachedProvenance::from_lookup(lookup, resolved_at));
                }
            }
        }

        self.entries.lock().map_or_else(
            |_| HashMap::new(),
            |entries| {
                paths
                    .into_iter()
                    .map(|path| {
                        let provenance = entries
                            .get(&path)
                            .map_or(InstallProvenance::Unknown, CachedProvenance::provenance);
                        (path, provenance)
                    })
                    .collect()
            },
        )
    }

    pub(super) fn resolve_one(&self, path: &Path) -> InstallProvenance {
        self.resolve_many([path.to_path_buf()])
            .remove(path)
            .unwrap_or(InstallProvenance::Unknown)
    }
}

fn detect_package_provider() -> Option<PackageProviderCommand> {
    [
        ("pacman", PackageProvider::Pacman),
        ("dpkg-query", PackageProvider::Dpkg),
        ("rpm", PackageProvider::Rpm),
    ]
    .into_iter()
    .find_map(|(program, provider)| {
        let executable = unixnotis_core::util::trusted_system_program_path(program)?;
        let evidence = executable_evidence_for_path(&executable)?;
        // Provider output affects attribution, so user-writable commands are never accepted
        (evidence.identity.is_system_managed() && evidence.identity.is_executable_regular())
            .then_some(PackageProviderCommand {
                provider,
                executable: evidence.canonical_path,
            })
    })
}

fn query_package_ownership(
    provider: &PackageProviderCommand,
    paths: &[PathBuf],
) -> HashMap<PathBuf, OwnershipLookup> {
    match provider.provider {
        PackageProvider::Pacman => query_in_chunks(provider, paths, &["-Qo"], parse_pacman_output),
        PackageProvider::Dpkg => query_in_chunks(provider, paths, &["--search"], parse_dpkg_output),
        // RPM output does not retain each selector, so bounded workers query paths separately
        PackageProvider::Rpm => query_rpm_ownership(provider, paths),
    }
}

fn query_in_chunks(
    provider: &PackageProviderCommand,
    paths: &[PathBuf],
    arguments: &[&str],
    parser: OwnershipOutputParser,
) -> HashMap<PathBuf, OwnershipLookup> {
    let mut resolved = HashMap::new();
    let mut remaining = paths;
    while remaining.split_first().is_some() {
        // A one-path floor preserves progress even if a future chunk policy returns zero
        let chunk_len = ownership_chunk_len(remaining).max(1).min(remaining.len());
        let (chunk, next) = remaining.split_at(chunk_len);
        let mut command = Command::new(&provider.executable);
        command
            .args(arguments)
            .args(chunk)
            .env_clear()
            .env("LC_ALL", "C");
        match run_package_query(&mut command, MAX_OWNERSHIP_OUTPUT_BYTES) {
            Ok(output) => {
                let parsed = parser(&output.stdout, chunk, provider.provider);
                for path in chunk {
                    let lookup = parsed.get(path).cloned().map_or_else(
                        || {
                            if output.status.success() && output.stdout.is_empty() {
                                OwnershipLookup::Negative(NegativeCause::NotOwned)
                            } else if output.status.success() {
                                OwnershipLookup::Negative(NegativeCause::MalformedOutput)
                            } else {
                                OwnershipLookup::Negative(NegativeCause::ProviderFailure)
                            }
                        },
                        OwnershipLookup::Known,
                    );
                    resolved.insert(path.clone(), lookup);
                }
            }
            Err(error) => {
                let cause = error.negative_cause();
                resolved.extend(
                    chunk
                        .iter()
                        .cloned()
                        .map(|path| (path, OwnershipLookup::Negative(cause))),
                );
            }
        }
        remaining = next;
    }
    resolved
}

fn ownership_chunk_len(paths: &[PathBuf]) -> usize {
    let mut bytes = 0_usize;
    let mut end = 0_usize;
    while end < paths.len() && end < MAX_COMMAND_PATHS {
        let next = paths[end].as_os_str().as_bytes().len().saturating_add(1);
        // The first path always advances so even an oversized selector cannot stall the scan
        if end > 0 && bytes.saturating_add(next) > MAX_COMMAND_ARGUMENT_BYTES {
            break;
        }
        bytes = bytes.saturating_add(next);
        end = end.saturating_add(1);
    }
    end
}

type OwnershipOutputParser =
    fn(&[u8], &[PathBuf], PackageProvider) -> HashMap<PathBuf, InstallProvenance>;

fn parse_pacman_output(
    output: &[u8],
    paths: &[PathBuf],
    provider: PackageProvider,
) -> HashMap<PathBuf, InstallProvenance> {
    let expected = paths
        .iter()
        .map(|path| (path.as_os_str().as_bytes(), path))
        .collect::<HashMap<_, _>>();
    output
        .split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let marker = b" is owned by ";
            let position = line
                .windows(marker.len())
                .position(|window| window == marker)?;
            let path = expected.get(&line[..position])?;
            let package = line.get(position.saturating_add(marker.len())..)?;
            let package = package.split(|byte| *byte == b' ').next()?;
            package_provenance(provider, package).map(|owner| ((*path).clone(), owner))
        })
        .collect()
}

fn parse_dpkg_output(
    output: &[u8],
    paths: &[PathBuf],
    provider: PackageProvider,
) -> HashMap<PathBuf, InstallProvenance> {
    let expected = paths
        .iter()
        .map(|path| (path.as_os_str().as_bytes(), path))
        .collect::<HashMap<_, _>>();
    output
        .split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let position = line.windows(2).rposition(|window| window == b": ")?;
            let package = line.get(..position)?.split(|byte| *byte == b',').next()?;
            let path = expected.get(line.get(position.saturating_add(2)..)?)?;
            package_provenance(provider, package).map(|owner| ((*path).clone(), owner))
        })
        .collect()
}

fn query_rpm_ownership(
    provider: &PackageProviderCommand,
    paths: &[PathBuf],
) -> HashMap<PathBuf, OwnershipLookup> {
    query_rpm_ownership_with(paths, RPM_TOTAL_QUERY_TIMEOUT, &|path, timeout| {
        query_rpm_owner(provider, path, timeout)
    })
}

fn query_rpm_ownership_with<Query>(
    paths: &[PathBuf],
    total_timeout: Duration,
    query: &Query,
) -> HashMap<PathBuf, OwnershipLookup>
where
    Query: Fn(&Path, Duration) -> OwnershipLookup + Sync,
{
    let bounded_len = paths.len().min(MAX_RPM_QUERY_PATHS);
    let bounded = &paths[..bounded_len];
    let next = AtomicUsize::new(0);
    let results = Mutex::new(HashMap::with_capacity(bounded_len));
    let deadline = Instant::now()
        .checked_add(total_timeout)
        .unwrap_or_else(Instant::now);
    let worker_count = bounded_len.min(MAX_RPM_QUERY_WORKERS);

    std::thread::scope(|scope| {
        let mut workers = Vec::with_capacity(worker_count);
        for worker in 0..worker_count {
            let spawn = std::thread::Builder::new()
                .name(format!("unixnotis-rpm-owner-{worker}"))
                .spawn_scoped(scope, || loop {
                    let path_index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = bounded.get(path_index) else {
                        break;
                    };
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    let lookup = query(path, remaining.min(PACKAGE_QUERY_TIMEOUT));
                    if let Ok(mut results) = results.lock() {
                        results.insert(path.clone(), lookup);
                    }
                });
            if let Ok(worker) = spawn {
                workers.push(worker);
            }
        }
        for worker in workers {
            let _worker_result = worker.join();
        }
    });

    results.into_inner().unwrap_or_default()
}

fn query_rpm_owner(
    provider: &PackageProviderCommand,
    path: &Path,
    timeout: Duration,
) -> OwnershipLookup {
    let mut command = Command::new(&provider.executable);
    command
        .args(["-qf", "--queryformat", "%{NAME}\n"])
        .arg(path)
        .env_clear()
        .env("LC_ALL", "C");
    let output = match run_package_query_with_timeout(
        &mut command,
        MAX_PACKAGE_ID_BYTES.saturating_add(1),
        timeout,
    ) {
        Ok(output) => output,
        Err(error) => return OwnershipLookup::Negative(error.negative_cause()),
    };
    if !output.status.success() {
        return OwnershipLookup::Negative(NegativeCause::ProviderFailure);
    }
    let package = output.stdout.strip_suffix(b"\n").unwrap_or(&output.stdout);
    if package.is_empty() {
        return OwnershipLookup::Negative(NegativeCause::NotOwned);
    }
    package_provenance(provider.provider, package).map_or(
        OwnershipLookup::Negative(NegativeCause::MalformedOutput),
        OwnershipLookup::Known,
    )
}

fn package_provenance(provider: PackageProvider, package: &[u8]) -> Option<InstallProvenance> {
    if package.is_empty()
        || package.len() > MAX_PACKAGE_ID_BYTES
        || package
            .iter()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return None;
    }
    Some(InstallProvenance::Package {
        provider,
        package_id: std::str::from_utf8(package).ok()?.to_string(),
    })
}

#[derive(Debug)]
struct PackageQueryOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PackageQueryFailure {
    Spawn,
    Wait,
    Timeout,
    Reader,
    PipeDrainTimeout,
    OutputLimit,
}

impl PackageQueryFailure {
    const fn negative_cause(self) -> NegativeCause {
        match self {
            Self::Timeout | Self::PipeDrainTimeout => NegativeCause::Timeout,
            Self::OutputLimit => NegativeCause::MalformedOutput,
            Self::Spawn | Self::Wait | Self::Reader => NegativeCause::ProcessTermination,
        }
    }
}

fn run_package_query(
    command: &mut Command,
    output_limit: usize,
) -> Result<PackageQueryOutput, PackageQueryFailure> {
    run_package_query_with_timeout(command, output_limit, PACKAGE_QUERY_TIMEOUT)
}

fn run_package_query_with_timeout(
    command: &mut Command,
    output_limit: usize,
    timeout: Duration,
) -> Result<PackageQueryOutput, PackageQueryFailure> {
    // A provider may launch helpers that keep the output pipe open after its leader exits
    command.process_group(0);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_error| PackageQueryFailure::Spawn)?;
    // The child is its new process-group leader because process_group received zero
    let process_group = Pid::from_child(&child);
    let Some(stdout) = child.stdout.take() else {
        terminate_package_query(&mut child, process_group);
        return Err(PackageQueryFailure::Reader);
    };
    let (reader_tx, reader_rx) = mpsc::sync_channel(1);
    let reader = std::thread::Builder::new()
        .name("unixnotis-package-output".to_string())
        .spawn(move || {
            let limit = u64::try_from(output_limit)
                .unwrap_or(u64::MAX)
                .saturating_add(1);
            let mut output = Vec::new();
            let read_result = stdout.take(limit).read_to_end(&mut output);
            let _send_result = reader_tx.send(read_result.map(|_bytes| output));
        })
        .map_err(|_error| {
            terminate_package_query(&mut child, process_group);
            PackageQueryFailure::Reader
        })?;
    // The result channel owns completion; dropping the handle avoids every unbounded join path
    drop(reader);

    let started = Instant::now();
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        Ok(None) => {
            terminate_package_query(&mut child, process_group);
            return Err(PackageQueryFailure::Timeout);
        }
        Err(_error) => {
            terminate_package_query(&mut child, process_group);
            return Err(PackageQueryFailure::Wait);
        }
    };
    let remaining = timeout.saturating_sub(started.elapsed());
    let drain_timeout = remaining.min(PACKAGE_PIPE_DRAIN_TIMEOUT);
    let stdout = match reader_rx.recv_timeout(drain_timeout) {
        Ok(Ok(stdout)) => stdout,
        Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
            return Err(PackageQueryFailure::Reader);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // The leader exited, so only inherited pipe holders remain in its process group
            let _kill_result = kill_process_group(process_group, Signal::KILL);
            return Err(PackageQueryFailure::PipeDrainTimeout);
        }
    };
    if stdout.len() > output_limit {
        return Err(PackageQueryFailure::OutputLimit);
    }
    Ok(PackageQueryOutput { status, stdout })
}

fn terminate_package_query(child: &mut std::process::Child, process_group: Pid) {
    // Group termination closes ordinary inherited pipes while the bounded reap avoids startup hangs
    if kill_process_group(process_group, Signal::KILL).is_err() {
        let _kill_result = child.kill();
    }
    let _wait_result = child.wait_timeout(PACKAGE_PIPE_DRAIN_TIMEOUT);
}

#[cfg(test)]
mod tests;
