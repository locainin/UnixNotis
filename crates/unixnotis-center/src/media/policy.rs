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
    if !remote_https_url_allowed(&url) {
        return None;
    }
    // A remote art URL must have a network host
    let host = url.host()?;
    if matches!(host, Host::Domain(domain) if domain.trim_end_matches('.').eq_ignore_ascii_case("localhost") || domain.trim_end_matches('.').to_ascii_lowercase().ends_with(".localhost"))
    {
        return None;
    }
    if matches!(host, Host::Ipv4(addr) if !is_public_ip(IpAddr::V4(addr)))
        || matches!(host, Host::Ipv6(addr) if !is_public_ip(IpAddr::V6(addr)))
    {
        return None;
    }
    Some(url)
}

pub fn remote_https_url_allowed(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.port_or_known_default() == Some(443)
}

pub fn is_public_ip(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(addr) => ipv4_is_public(addr.octets()),
        IpAddr::V6(addr) => ipv6_is_public(addr.segments()),
    }
}

fn ipv4_is_public([first, second, third, _fourth]: [u8; 4]) -> bool {
    // Reject non-routable, local, documentation, benchmark, multicast, and reserved ranges
    !(first == 0
        || first == 10
        || first == 127
        || (first == 100 && (64..=127).contains(&second))
        || (first == 169 && second == 254)
        || (first == 172 && (16..=31).contains(&second))
        || (first == 192 && second == 0 && third == 0)
        || (first == 192 && second == 0 && third == 2)
        || (first == 192 && second == 88 && third == 99)
        || (first == 192 && second == 168)
        || (first == 198 && matches!(second, 18 | 19))
        || (first == 198 && second == 51 && third == 100)
        || (first == 203 && second == 0 && third == 113)
        || first >= 224)
}

fn ipv6_is_public(segments: [u16; 8]) -> bool {
    // Mapped addresses inherit the IPv4 destination policy
    if segments[..5] == [0, 0, 0, 0, 0] && segments[5] == 0xffff {
        let high = segments[6].to_be_bytes();
        let low = segments[7].to_be_bytes();
        return ipv4_is_public([high[0], high[1], low[0], low[1]]);
    }

    // Current globally routed unicast space is 2000::/3
    if !(0x2000..=0x3fff).contains(&segments[0]) {
        return false;
    }
    // IETF assignments, documentation blocks, and the expanded documentation prefix stay local
    if (segments[0] == 0x2001 && (segments[1] <= 0x01ff || segments[1] == 0x0db8))
        || (segments[0] == 0x3fff && segments[1] <= 0x0fff)
    {
        return false;
    }
    if segments[0] == 0x2002 {
        // 6to4 embeds its eventual IPv4 destination in the next two segments
        let high = segments[1].to_be_bytes();
        let low = segments[2].to_be_bytes();
        return ipv4_is_public([high[0], high[1], low[0], low[1]]);
    }
    true
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
#[path = "tests/policy.rs"]
mod tests;
