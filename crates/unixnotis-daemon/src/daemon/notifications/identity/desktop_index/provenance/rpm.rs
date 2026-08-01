//! Bounded RPM ownership queries

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::cache::{NegativeCause, OwnershipLookup};
use super::process::run_package_query_with_timeout;
use super::query::PackageProviderCommand;

const MAX_RPM_QUERY_PATHS: usize = 4_096;
const MAX_RPM_QUERY_WORKERS: usize = 8;
const RPM_TOTAL_QUERY_TIMEOUT: Duration = Duration::from_secs(2);
const PACKAGE_QUERY_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_PACKAGE_ID_BYTES: usize = 256;

pub(super) fn query_rpm_ownership(
    provider: &PackageProviderCommand,
    paths: &[PathBuf],
) -> HashMap<PathBuf, OwnershipLookup> {
    query_rpm_ownership_with(paths, RPM_TOTAL_QUERY_TIMEOUT, &|path, timeout| {
        query_rpm_owner(provider, path, timeout)
    })
}

pub(super) fn query_rpm_ownership_with<Query>(
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

pub(super) fn query_rpm_owner(
    provider: &PackageProviderCommand,
    path: &Path,
    timeout: Duration,
) -> OwnershipLookup {
    let mut command = std::process::Command::new(&provider.executable);
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
    super::query::package_provenance(provider.provider, package).map_or(
        OwnershipLookup::Negative(NegativeCause::MalformedOutput),
        OwnershipLookup::Known,
    )
}
