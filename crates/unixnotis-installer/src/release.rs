//! Release version and update status helpers

#[cfg(not(test))]
use std::process::Command;

#[cfg(not(test))]
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/locainin/UnixNotis/releases/latest";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseStatus {
    pub current: String,
    pub latest: Option<String>,
    pub state: ReleaseUpdateState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseUpdateState {
    UpToDate,
    UpdateAvailable,
    Unknown,
}

impl ReleaseStatus {
    pub fn detect() -> Self {
        // Current version comes from Cargo so source and release builds agree
        let current = current_version_tag();

        #[cfg(test)]
        {
            // Unit tests must not depend on GitHub, network access, or curl timing
            Self {
                current,
                latest: None,
                state: ReleaseUpdateState::Unknown,
            }
        }

        #[cfg(not(test))]
        match fetch_latest_release_tag() {
            Ok(latest) => {
                // GitHub can return any valid tag name, but the UI only treats stable tags as updates
                // Invalid or unexpected tag formats fail closed as "not newer"
                let state = if release_tag_is_newer(&latest, &current) {
                    ReleaseUpdateState::UpdateAvailable
                } else {
                    ReleaseUpdateState::UpToDate
                };
                Self {
                    current,
                    latest: Some(latest),
                    state,
                }
            }
            Err(_) => Self {
                // Update checks are informational and should never block installer startup
                current,
                latest: None,
                state: ReleaseUpdateState::Unknown,
            },
        }
    }

    #[cfg(test)]
    pub fn current_only() -> Self {
        // Render tests use this helper so the welcome screen stays deterministic
        Self {
            current: current_version_tag(),
            latest: None,
            state: ReleaseUpdateState::Unknown,
        }
    }

    pub fn display_line(&self) -> String {
        // Keep the line compact because it sits in the installer status panel
        match (self.state, self.latest.as_deref()) {
            (ReleaseUpdateState::UpdateAvailable, Some(latest)) => {
                format!("{} installed; {latest} available", self.current)
            }
            (ReleaseUpdateState::UpToDate, Some(latest)) => {
                format!("{} installed; latest release is {latest}", self.current)
            }
            _ => format!("{} installed; update check unavailable", self.current),
        }
    }
}

fn current_version_tag() -> String {
    // GitHub release tags use a v-prefix while Cargo stores the numeric version
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

#[cfg(not(test))]
fn fetch_latest_release_tag() -> Result<String, String> {
    // GitHub's latest-release endpoint returns the newest non-draft, non-prerelease release
    // Curl is used instead of adding an HTTP client dependency to the installer binary
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "2",
            // Keep requests explicit so GitHub API behavior does not depend on global curl config
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "X-GitHub-Api-Version: 2022-11-28",
            LATEST_RELEASE_URL,
        ])
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        // Curl failure detail is useful for logs while the UI only needs unknown status
        return Err(format!("curl exited with {}", output.status));
    }

    latest_tag_from_json(&output.stdout)
}

fn latest_tag_from_json(bytes: &[u8]) -> Result<String, String> {
    // Parse only the single field needed for the TUI instead of binding to the full API shape
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|err| err.to_string())?;
    value
        .get("tag_name")
        .and_then(|tag| tag.as_str())
        // Empty tags are treated like malformed API data, not like "no update"
        .filter(|tag| !tag.trim().is_empty())
        .map(|tag| tag.trim().to_string())
        .ok_or_else(|| "latest release response did not include tag_name".to_string())
}

fn release_tag_is_newer(latest: &str, current: &str) -> bool {
    // Unknown tag formats are treated as non-updates so prereleases cannot surprise users
    match (parse_version_tag(latest), parse_version_tag(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

fn parse_version_tag(tag: &str) -> Option<(u32, u32, u32)> {
    // Only stable semver tags are supported by the installer update indicator
    let trimmed = tag.trim().trim_start_matches('v');
    let mut parts = trimmed.split('.');
    // Tuple comparison keeps major/minor/patch ordering correct without string sorting
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = parts.next()?.parse::<u32>().ok()?;
    // Stable release tags must stay simple so update checks do not misread prerelease text
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

#[cfg(test)]
#[path = "tests/release.rs"]
mod tests;
