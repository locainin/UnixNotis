//! Release version and update status helpers

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/locainin/UnixNotis/releases/latest";
const MAX_RELEASE_RESPONSE_BYTES: &str = "65536";

use crate::system_tools;

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
        Self::detect_with(fetch_latest_release_tag)
    }

    fn detect_with(fetch_latest: impl FnOnce() -> Result<String, String>) -> Self {
        // Current version comes from Cargo so source and release builds agree
        let current = current_version_tag();

        match fetch_latest() {
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

    pub fn current_only() -> Self {
        // Startup uses this lightweight value while the network check runs later
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

fn fetch_latest_release_tag() -> Result<String, String> {
    // GitHub's latest-release endpoint returns the newest non-draft, non-prerelease release
    // Curl is used instead of adding an HTTP client dependency to the installer binary
    let output = system_tools::command("curl")
        .map_err(|err| err.to_string())?
        .args(latest_release_curl_args())
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        // Curl failure detail is useful for logs while the UI only needs unknown status
        return Err(format!("curl exited with {}", output.status));
    }

    latest_tag_from_json(&output.stdout)
}

const fn latest_release_curl_args() -> [&'static str; 14] {
    [
        // Disable .curlrc so local flags cannot rewrite the release-check request
        "-q",
        "-fsSL",
        "--proto",
        "=https",
        "--tlsv1.2",
        "--max-time",
        "2",
        // The latest-release JSON is tiny; this prevents accidental large captures
        "--max-filesize",
        MAX_RELEASE_RESPONSE_BYTES,
        "-H",
        "Accept: application/vnd.github+json",
        "-H",
        "X-GitHub-Api-Version: 2022-11-28",
        LATEST_RELEASE_URL,
    ]
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
        (Some(latest), Some(current)) => latest.cmp_precedence(&current).is_gt(),
        _ => false,
    }
}

fn parse_version_tag(tag: &str) -> Option<semver::Version> {
    // The SemVer implementation keeps build metadata out of precedence comparisons
    semver::Version::parse(tag.trim().trim_start_matches('v')).ok()
}

#[cfg(test)]
#[path = "release/tests.rs"]
mod tests;
