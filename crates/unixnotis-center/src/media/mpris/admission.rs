//! Player allowlist, denylist, and browser-name admission

use std::fs;
use std::os::unix::fs::MetadataExt;

use unixnotis_core::{MediaConfig, MediaLocalArtPolicy, MediaRemoteArtPolicy};

pub(super) fn detect_browser_family(
    identity: &str,
    bus_name: &str,
    browser_tokens: &[String],
) -> Option<String> {
    if browser_tokens.is_empty() {
        return None;
    }
    // The bus name is the most stable source when a browser opens many players
    let bus_lower = bus_name.to_lowercase();
    if let Some(family) = browser_family_from_value(&bus_lower, browser_tokens) {
        return Some(family);
    }
    let identity_lower = identity.to_lowercase();
    browser_family_from_value(&identity_lower, browser_tokens).or_else(|| {
        // Browser-like identities sometimes expose their family only in the MPRIS suffix
        if !identity_lower.contains("browser") {
            return None;
        }
        mpris_suffix(&bus_lower).map(std::string::ToString::to_string)
    })
}

pub(super) fn remote_art_allowed(
    browser_family: Option<&str>,
    owner_executable: Option<&str>,
    policy: MediaRemoteArtPolicy,
) -> bool {
    // A missing owner executable means the bus owner is not concrete enough to trust
    let has_owner = owner_executable.is_some_and(|value| !value.trim().is_empty());
    if !has_owner {
        return false;
    }
    match policy {
        MediaRemoteArtPolicy::Disabled => false,
        // Browsers stay opt-in because webpage metadata can choose the art URL
        MediaRemoteArtPolicy::NativeOnly => browser_family.is_none(),
        MediaRemoteArtPolicy::BrowsersToo => true,
    }
}

pub(super) fn local_art_allowed(
    browser_family: Option<&str>,
    owner_executable: Option<&str>,
    policy: MediaLocalArtPolicy,
    _allowlist: &[String],
    executable_allowlist: &[String],
) -> bool {
    // A missing owner executable means the bus owner is not concrete enough to trust
    let has_owner = owner_executable.is_some_and(|value| !value.trim().is_empty());
    if !has_owner {
        return false;
    }
    match policy {
        MediaLocalArtPolicy::Disabled => false,
        MediaLocalArtPolicy::ExactExecutableOnly => {
            // Browser bridges can direct the renderer to arbitrary host files via mpris:artUrl.
            // Only native players (non-browser) with an allowlist-matched executable may name host files.
            browser_family.is_none()
                && is_executable_allowed(owner_executable.unwrap_or(""), executable_allowlist)
        }
        MediaLocalArtPolicy::AllAdmitted => {
            // Browser bridges can direct the renderer to arbitrary host files via mpris:artUrl.
            // Only native players (non-browser) may name host files for local artwork.
            browser_family.is_none()
        }
    }
}

fn is_executable_allowed(executable: &str, allowlist: &[String]) -> bool {
    if allowlist.is_empty() {
        return false;
    }
    // Compare device and inode of the owner executable against allowlisted executables
    // This prevents impersonation via executable name spoofing
    let owner_meta = match fs::metadata(executable) {
        Ok(meta) => meta,
        Err(_) => return false,
    };
    let owner_dev = owner_meta.dev();
    let owner_ino = owner_meta.ino();

    allowlist.iter().any(|allowed_path| {
        let allowed_meta = match fs::metadata(allowed_path) {
            Ok(meta) => meta,
            Err(_) => return false,
        };
        allowed_meta.dev() == owner_dev && allowed_meta.ino() == owner_ino
    })
}

pub(in crate::media) fn is_allowed_player(name: &str, config: &MediaConfig) -> bool {
    let lower = name.to_lowercase();
    if config.denylist.iter().any(|entry| lower.contains(entry)) {
        return false;
    }

    if !config.allowlist.is_empty() {
        return config.allowlist.iter().any(|entry| lower.contains(entry));
    }

    if !config.include_browsers && is_browser_name(&lower, &config.browser_tokens) {
        return false;
    }

    true
}

fn is_browser_name(lower: &str, browser_tokens: &[String]) -> bool {
    // Browser tokens match whole segments so short defaults do not overfire
    browser_tokens
        .iter()
        .any(|token| token_matches_segment(lower, token))
}

fn browser_family_from_value(value: &str, browser_tokens: &[String]) -> Option<String> {
    for token in browser_tokens {
        // Browser tokens should match name segments, not random inner substrings
        if token_matches_segment(value, token) {
            return Some(token.clone());
        }
    }
    None
}

fn token_matches_segment(value: &str, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }

    // Split on non-word separators so edge matches microsoft-edge but not knowledge
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|segment| segment == token)
}

fn mpris_suffix(bus_name: &str) -> Option<&str> {
    let suffix = bus_name.strip_prefix("org.mpris.mediaplayer2.")?;
    // The first segment is stable enough for family grouping across browser instances
    Some(suffix.split('.').next().unwrap_or(suffix))
}