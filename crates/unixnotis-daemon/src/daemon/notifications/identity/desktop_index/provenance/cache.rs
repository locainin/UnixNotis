//! Negative-result cache for package ownership lookups

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::query::{detect_package_provider, query_package_ownership};
use super::InstallProvenance;

const MAX_OWNERSHIP_PATHS: usize = 16_384;
pub(super) const TRANSIENT_NEGATIVE_TTL: Duration = Duration::from_secs(30);
pub(super) const NOT_OWNED_NEGATIVE_TTL: Duration = Duration::from_mins(5);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum NegativeCause {
    NotOwned,
    Timeout,
    ProviderFailure,
    MalformedOutput,
    ProcessTermination,
}

#[derive(Debug, Clone)]
pub(super) enum CachedProvenance {
    Known(InstallProvenance),
    Negative {
        retry_after: Instant,
        cause: NegativeCause,
    },
}

impl CachedProvenance {
    pub(super) fn from_lookup(lookup: OwnershipLookup, now: Instant) -> Self {
        match lookup {
            OwnershipLookup::Known(provenance) => Self::Known(provenance),
            OwnershipLookup::Negative(cause) => Self::Negative {
                retry_after: now.checked_add(negative_ttl(cause)).unwrap_or(now),
                cause,
            },
        }
    }

    pub(super) fn needs_refresh(&self, now: Instant) -> bool {
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

    pub(super) fn provenance(&self) -> InstallProvenance {
        match self {
            Self::Known(provenance) => provenance.clone(),
            Self::Negative { .. } => InstallProvenance::Unknown,
        }
    }
}

fn negative_ttl(cause: NegativeCause) -> Duration {
    match cause {
        NegativeCause::NotOwned => NOT_OWNED_NEGATIVE_TTL,
        NegativeCause::Timeout
        | NegativeCause::ProviderFailure
        | NegativeCause::MalformedOutput
        | NegativeCause::ProcessTermination => TRANSIENT_NEGATIVE_TTL,
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum OwnershipLookup {
    Known(InstallProvenance),
    Negative(NegativeCause),
}

#[derive(Debug, Default)]
pub(in crate::daemon::notifications::identity) struct PackageOwnershipCache {
    provider: OnceLock<Option<super::query::PackageProviderCommand>>,
    pub(super) entries: Mutex<HashMap<PathBuf, CachedProvenance>>,
}

impl PackageOwnershipCache {
    pub(in crate::daemon::notifications::identity) fn resolve_many(
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

    pub(in crate::daemon::notifications::identity) fn resolve_one(
        &self,
        path: &Path,
    ) -> InstallProvenance {
        self.resolve_many([path.to_path_buf()])
            .remove(path)
            .unwrap_or(InstallProvenance::Unknown)
    }
}
