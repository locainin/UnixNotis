//! Immutable installation ownership used by desktop attribution

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use super::super::executable::executable_evidence_for_path;
use wait_timeout::ChildExt;

const MAX_OWNERSHIP_PATHS: usize = 16_384;
const MAX_COMMAND_ARGUMENT_BYTES: usize = 192 * 1024;
const MAX_COMMAND_PATHS: usize = 4_096;
const MAX_OWNERSHIP_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_PACKAGE_ID_BYTES: usize = 256;
const PACKAGE_QUERY_TIMEOUT: Duration = Duration::from_secs(1);

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

#[derive(Debug, Default)]
pub(super) struct PackageOwnershipCache {
    provider: OnceLock<Option<PackageProviderCommand>>,
    entries: Mutex<HashMap<PathBuf, InstallProvenance>>,
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
        let missing = self.entries.lock().map_or_else(
            |_| paths.iter().cloned().collect::<Vec<_>>(),
            |entries| {
                paths
                    .iter()
                    .filter(|path| !entries.contains_key(*path))
                    .cloned()
                    .collect::<Vec<_>>()
            },
        );

        if !missing.is_empty() {
            let resolved = self
                .provider
                .get_or_init(detect_package_provider)
                .as_ref()
                .map_or_else(HashMap::new, |provider| {
                    query_package_ownership(provider, &missing)
                });
            if let Ok(mut entries) = self.entries.lock() {
                for path in missing {
                    entries.insert(
                        path.clone(),
                        resolved
                            .get(&path)
                            .cloned()
                            .unwrap_or(InstallProvenance::Unknown),
                    );
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
                            .cloned()
                            .unwrap_or(InstallProvenance::Unknown);
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
) -> HashMap<PathBuf, InstallProvenance> {
    match provider.provider {
        PackageProvider::Pacman => query_in_chunks(provider, paths, &["-Qo"], parse_pacman_output),
        PackageProvider::Dpkg => query_in_chunks(provider, paths, &["--search"], parse_dpkg_output),
        PackageProvider::Rpm => {
            // RPM does not retain the queried path in batch output
            // Single-path lookups remain safe while bulk indexing fails closed
            if paths.len() == 1 {
                query_rpm_owner(provider, &paths[0])
                    .map(|owner| HashMap::from([(paths[0].clone(), owner)]))
                    .unwrap_or_default()
            } else {
                HashMap::new()
            }
        }
    }
}

fn query_in_chunks(
    provider: &PackageProviderCommand,
    paths: &[PathBuf],
    arguments: &[&str],
    parser: OwnershipOutputParser,
) -> HashMap<PathBuf, InstallProvenance> {
    let mut resolved = HashMap::new();
    let mut start = 0;
    while start < paths.len() {
        let mut bytes = 0_usize;
        let mut end = start;
        while end < paths.len() && end.saturating_sub(start) < MAX_COMMAND_PATHS {
            let next = paths[end].as_os_str().as_bytes().len().saturating_add(1);
            if end > start && bytes.saturating_add(next) > MAX_COMMAND_ARGUMENT_BYTES {
                break;
            }
            bytes = bytes.saturating_add(next);
            end = end.saturating_add(1);
        }
        let chunk = &paths[start..end];
        let mut command = Command::new(&provider.executable);
        command
            .args(arguments)
            .args(chunk)
            .env_clear()
            .env("LC_ALL", "C");
        if let Some(output) = run_package_query(&mut command, MAX_OWNERSHIP_OUTPUT_BYTES) {
            resolved.extend(parser(&output.stdout, chunk, provider.provider));
        }
        start = end;
    }
    resolved
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

fn query_rpm_owner(provider: &PackageProviderCommand, path: &Path) -> Option<InstallProvenance> {
    let mut command = Command::new(&provider.executable);
    command
        .args(["-qf", "--queryformat", "%{NAME}\n"])
        .arg(path)
        .env_clear()
        .env("LC_ALL", "C");
    let output = run_package_query(&mut command, MAX_PACKAGE_ID_BYTES.saturating_add(1))?;
    if !output.status.success() || output.stdout.len() > MAX_PACKAGE_ID_BYTES.saturating_add(1) {
        return None;
    }
    package_provenance(
        provider.provider,
        output.stdout.strip_suffix(b"\n").unwrap_or(&output.stdout),
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

fn run_package_query(command: &mut Command, output_limit: usize) -> Option<PackageQueryOutput> {
    run_package_query_with_timeout(command, output_limit, PACKAGE_QUERY_TIMEOUT)
}

fn run_package_query_with_timeout(
    command: &mut Command,
    output_limit: usize,
    timeout: Duration,
) -> Option<PackageQueryOutput> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let limit = u64::try_from(output_limit)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let mut output = Vec::new();
        stdout.take(limit).read_to_end(&mut output).ok()?;
        Some(output)
    });

    let status = if let Some(status) = child.wait_timeout(timeout).ok()? {
        status
    } else {
        let _kill_result = child.kill();
        let _wait_result = child.wait();
        let _reader_result = reader.join();
        return None;
    };
    let stdout = reader.join().ok()??;
    if stdout.len() > output_limit {
        return None;
    }
    Some(PackageQueryOutput { status, stdout })
}

#[cfg(test)]
#[path = "tests/provenance.rs"]
mod tests;
