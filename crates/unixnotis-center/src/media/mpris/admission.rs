//! Player allowlist, denylist, and browser-name admission

use std::fs::File;
use std::io::Read;
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
    owner_pid: Option<u32>,
    policy: MediaLocalArtPolicy,
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
                && owner_pid.is_some_and(|pid| {
                    is_executable_allowed(pid, owner_executable.unwrap_or(""), executable_allowlist)
                })
        }
        MediaLocalArtPolicy::AllAdmitted => {
            // Browser bridges can direct the renderer to arbitrary host files via mpris:artUrl.
            // Only native players (non-browser) may name host files for local artwork.
            browser_family.is_none()
        }
    }
}

const MAX_EXECUTABLE_FINGERPRINT_BYTES: u64 = 512 * 1024 * 1024;

fn is_executable_allowed(pid: u32, executable: &str, allowlist: &[String]) -> bool {
    if allowlist.is_empty() {
        return false;
    }
    if executable.trim().is_empty() {
        return false;
    }

    // Open the kernel-reported executable object before examining the allowlist paths
    let owner_file = match File::open(format!("/proc/{pid}/exe")) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let owner_meta = match owner_file.metadata() {
        Ok(meta) => meta,
        Err(_) => return false,
    };
    // Large system binaries only need stable descriptor identity; user-owned files
    // also require a bounded content fingerprint to cover in-place replacement
    let needs_digest = owner_meta.uid() != 0
        || allowlist.iter().any(|path| {
            File::open(path)
                .and_then(|file| file.metadata())
                .is_ok_and(|metadata| metadata.uid() != 0)
        });
    let owner_digest = if needs_digest {
        match executable_digest(owner_file) {
            Some(digest) => Some(digest),
            None => return false,
        }
    } else {
        None
    };
    let owner_identity = (owner_meta.dev(), owner_meta.ino());

    allowlist.iter().any(|allowed_path| {
        let allowed_file = match File::open(allowed_path) {
            Ok(file) => file,
            Err(_) => return false,
        };
        let allowed_meta = match allowed_file.metadata() {
            Ok(meta) => meta,
            Err(_) => return false,
        };
        if (allowed_meta.dev(), allowed_meta.ino()) != owner_identity {
            return false;
        }
        if owner_meta.uid() == 0 && allowed_meta.uid() == 0 {
            return true;
        }
        let Some(owner_digest) = owner_digest else {
            return false;
        };
        executable_digest(allowed_file).is_some_and(|allowed_digest| allowed_digest == owner_digest)
    })
}

fn executable_digest(mut file: File) -> Option<[u8; 32]> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            return Some(*hasher.finalize().as_bytes());
        }
        total = total.checked_add(u64::try_from(read).ok()?)?;
        if total > MAX_EXECUTABLE_FINGERPRINT_BYTES {
            return None;
        }
        hasher.update(&buffer[..read]);
    }
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
