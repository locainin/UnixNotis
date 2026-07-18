//! Local and remote artwork source normalization

use std::net::IpAddr;
use std::path::PathBuf;

use url::{Host, Url};

use super::network_policy::{is_public_ip, remote_https_url_allowed};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaArtSource {
    LocalFile(PathBuf),
    RemoteHttps(Url),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum MediaArtKey {
    Local(PathBuf),
    Remote(Url),
}

impl MediaArtSource {
    pub(crate) fn stable_key(&self) -> MediaArtKey {
        match self {
            // Native paths retain every platform byte instead of using a display conversion
            Self::LocalFile(path) => MediaArtKey::Local(path.clone()),
            // URL keeps its parsed normalized identity and cannot overlap the local variant
            Self::RemoteHttps(url) => MediaArtKey::Remote(url.clone()),
        }
    }
}

pub(in crate::media) fn normalize_art_source(
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
