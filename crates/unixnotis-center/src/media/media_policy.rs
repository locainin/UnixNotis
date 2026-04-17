use std::net::IpAddr;
use std::path::PathBuf;

use unixnotis_core::MediaRemoteArtPolicy;
use url::{Host, Url};

use super::MediaArtSource;

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
        // Browser-ish identities sometimes hide only in the MPRIS suffix
        if !identity_lower.contains("browser") {
            return None;
        }
        mpris_suffix(&bus_lower).map(|suffix| suffix.to_string())
    })
}

pub(super) fn remote_art_allowed(
    browser_family: Option<&str>,
    owner_executable: Option<&str>,
    policy: MediaRemoteArtPolicy,
) -> bool {
    // A missing owner executable means the bus owner is not concrete enough to trust
    let has_owner = owner_executable
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
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

pub(super) fn normalize_art_source(
    value: &str,
    allow_remote_https: bool,
) -> Option<MediaArtSource> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Local files stay available for native players like mpv and smplayer
    if let Some(path) = normalize_local_file(trimmed) {
        return Some(MediaArtSource::LocalFile(path));
    }
    if !allow_remote_https {
        return None;
    }
    normalize_remote_https(trimmed).map(MediaArtSource::RemoteHttps)
}

fn normalize_local_file(value: &str) -> Option<PathBuf> {
    // Raw absolute paths are already local
    if value.starts_with('/') {
        return Some(PathBuf::from(value));
    }

    let url = Url::parse(value).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    // Only empty hosts and localhost are treated as native local files
    match url.host_str() {
        None => {}
        Some(host) if host.eq_ignore_ascii_case("localhost") => {}
        Some(_) => return None,
    }
    url.to_file_path().ok()
}

fn normalize_remote_https(value: &str) -> Option<Url> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    // A remote art URL must have a network host
    let host = url.host()?;
    if is_blocked_remote_host(&host) {
        return None;
    }
    Some(url)
}

fn is_blocked_remote_host(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(addr) => is_blocked_ip(&IpAddr::V4(*addr)),
        Host::Ipv6(addr) => is_blocked_ip(&IpAddr::V6(*addr)),
    }
}

fn is_blocked_ip(addr: &IpAddr) -> bool {
    match addr {
        IpAddr::V4(addr) => {
            addr.is_private()
                || addr.is_loopback()
                || addr.is_link_local()
                || addr.is_unspecified()
                || addr.is_broadcast()
        }
        IpAddr::V6(addr) => {
            addr.is_loopback()
                || addr.is_unspecified()
                || addr.is_unique_local()
                || addr.is_unicast_link_local()
        }
    }
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

pub(super) fn token_matches_segment(value: &str, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }

    // Split on non-word-ish separators so tokens like "edge" still match
    // "microsoft-edge", but not unrelated names like "knowledge"
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|segment| segment == token)
}

fn mpris_suffix(bus_name: &str) -> Option<&str> {
    let suffix = bus_name.strip_prefix("org.mpris.mediaplayer2.")?;
    // The first segment is stable enough for family grouping across browser instances
    Some(suffix.split('.').next().unwrap_or(suffix))
}

#[cfg(test)]
mod tests {
    use unixnotis_core::MediaRemoteArtPolicy;

    use super::{detect_browser_family, normalize_art_source, remote_art_allowed};
    use crate::media::MediaArtSource;

    #[test]
    fn normalize_art_source_keeps_local_files() {
        let source = normalize_art_source("file:///tmp/track%20art.png", false);
        assert!(matches!(source, Some(MediaArtSource::LocalFile(_))));
    }

    #[test]
    fn normalize_art_source_rejects_insecure_or_local_remote_targets() {
        assert!(normalize_art_source("http://example.com/art.png", true).is_none());
        assert!(normalize_art_source("https://127.0.0.1/art.png", true).is_none());
        assert!(normalize_art_source("https://localhost/art.png", true).is_none());
    }

    #[test]
    fn normalize_art_source_accepts_https_when_remote_is_allowed() {
        let source = normalize_art_source("https://example.com/art.png", true);
        assert!(matches!(source, Some(MediaArtSource::RemoteHttps(_))));
    }

    #[test]
    fn detect_browser_family_matches_bus_or_identity() {
        let tokens = vec!["firefox".to_string()];
        assert_eq!(
            detect_browser_family(
                "Firefox",
                "org.mpris.MediaPlayer2.firefox.instance",
                &tokens
            ),
            Some("firefox".to_string())
        );
    }

    #[test]
    fn remote_art_policy_blocks_browsers_by_default() {
        assert!(!remote_art_allowed(
            Some("firefox"),
            Some("/usr/bin/firefox"),
            MediaRemoteArtPolicy::NativeOnly
        ));
        assert!(remote_art_allowed(
            None,
            Some("/usr/bin/spotify"),
            MediaRemoteArtPolicy::NativeOnly
        ));
        assert!(!remote_art_allowed(
            None,
            None,
            MediaRemoteArtPolicy::BrowsersToo
        ));
    }

    #[test]
    fn detect_browser_family_does_not_match_inner_substrings() {
        let tokens = vec!["zen".to_string(), "edge".to_string()];

        assert_eq!(
            detect_browser_family("Zenity Helper", "org.mpris.MediaPlayer2.zenity", &tokens),
            None
        );
        assert_eq!(
            detect_browser_family(
                "Knowledge Player",
                "org.mpris.MediaPlayer2.knowledge",
                &tokens
            ),
            None
        );
        assert_eq!(
            detect_browser_family(
                "Microsoft Edge",
                "org.mpris.MediaPlayer2.microsoft-edge",
                &tokens
            ),
            Some("edge".to_string())
        );
    }
}
