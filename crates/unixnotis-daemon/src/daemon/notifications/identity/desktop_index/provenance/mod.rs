//! Immutable installation ownership used by desktop attribution

mod cache;
mod process;
mod query;
mod rpm;

pub(super) use cache::PackageOwnershipCache;

/// System database that established package ownership
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub(in crate::daemon::notifications) enum PackageProvider {
    Pacman,
    Dpkg,
    Rpm,
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

#[cfg(test)]
mod tests;
