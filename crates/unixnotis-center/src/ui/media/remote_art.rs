//! DNS-validated and address-pinned remote artwork downloads

use std::io::Read;
use std::net::IpAddr;
use std::process::{Command, Stdio};

use gio::prelude::ResolverExt;
use url::{Host, Url};

use crate::media::{is_public_ip, remote_https_url_allowed};

const MAX_REMOTE_ART_BYTES: usize = 2 * 1_024 * 1_024;
const MAX_REMOTE_ART_BYTES_ARG: &str = "2097152";
const MAX_REMOTE_ADDRESSES: usize = 16;
const REMOTE_ART_CONNECT_TIMEOUT_SECS: &str = "2";
const REMOTE_ART_TOTAL_TIMEOUT_SECS: &str = "3";

#[derive(Clone, Debug, PartialEq, Eq)]
struct PinnedRemoteTarget {
    url: Url,
    host: String,
    port: u16,
    addresses: Vec<IpAddr>,
    needs_resolve_override: bool,
}

pub(super) async fn read_remote_art(url: &Url, program: &std::path::Path) -> Option<Vec<u8>> {
    // DNS stays on the async resolver before the blocking transfer begins
    let target = resolve_remote_target(url).await?;
    // Own the program path so the worker never borrows GTK-thread state
    let program = program.to_path_buf();
    gio::spawn_blocking(move || {
        fetch_remote_art_output(remote_art_command(&target, &program), MAX_REMOTE_ART_BYTES)
    })
    .await
    .ok()?
}

async fn resolve_remote_target(url: &Url) -> Option<PinnedRemoteTarget> {
    // Recheck the parsed URL at the request boundary
    if !remote_https_url_allowed(url) {
        return None;
    }
    let host = url.host()?;
    match host {
        // Literal addresses are already pinned by the URL itself
        Host::Ipv4(address) => target_from_addresses(url, vec![IpAddr::V4(address)], false),
        Host::Ipv6(address) => target_from_addresses(url, vec![IpAddr::V6(address)], false),
        Host::Domain(domain) => {
            // Every answer is retained so one unsuitable destination rejects the request
            let resolved = gio::Resolver::default()
                .lookup_by_name_future(domain)
                .await
                .ok()?;
            let mut addresses = Vec::with_capacity(resolved.len());
            for address in resolved {
                addresses.push(address.to_string().parse::<IpAddr>().ok()?);
            }
            target_from_addresses(url, addresses, true)
        }
    }
}

fn target_from_addresses(
    url: &Url,
    mut addresses: Vec<IpAddr>,
    needs_resolve_override: bool,
) -> Option<PinnedRemoteTarget> {
    // The helper validates its own inputs so future callers cannot widen the policy
    if !remote_https_url_allowed(url)
        || addresses.is_empty()
        || addresses.len() > MAX_REMOTE_ADDRESSES
    {
        return None;
    }
    // One unsuitable answer rejects the whole hostname instead of letting resolver order choose policy
    if addresses.iter().any(|address| !is_public_ip(*address)) {
        return None;
    }
    // Stable ordering keeps the curl override deterministic across resolver implementations
    addresses.sort_unstable();
    addresses.dedup();
    Some(PinnedRemoteTarget {
        url: url.clone(),
        host: url.host_str()?.to_string(),
        port: url.port_or_known_default()?,
        addresses,
        needs_resolve_override,
    })
}

fn fetch_remote_art_output(mut command: Command, max_bytes: usize) -> Option<Vec<u8>> {
    // No inherited input or diagnostic stream can interfere with the GTK process
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let mut bytes = Vec::new();
    // One extra byte turns response growth into a bounded rejection
    let read_result = stdout
        .by_ref()
        .take(u64::try_from(max_bytes).ok()?.saturating_add(1))
        .read_to_end(&mut bytes);
    if read_result.is_err() || bytes.len() > max_bytes {
        stop_child(&mut child);
        return None;
    }
    // A complete body is usable only when the transfer process also succeeded
    let status = child.wait().ok()?;
    if !status.success() || bytes.is_empty() {
        return None;
    }
    Some(bytes)
}

fn remote_art_command(target: &PinnedRemoteTarget, program: &std::path::Path) -> Command {
    // Production supplies a trusted absolute program path instead of inherited PATH lookup
    let mut command = Command::new(program);
    command.args([
        // Keep this first so user curl configuration cannot alter the request
        "-q",
        "--fail",
        "--silent",
        "--globoff",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        // Redirect handling is enabled with a zero ceiling so every redirect fails immediately
        "--location",
        "--max-redirs",
        "0",
        "--tlsv1.2",
        "--connect-timeout",
        REMOTE_ART_CONNECT_TIMEOUT_SECS,
        "--max-time",
        REMOTE_ART_TOTAL_TIMEOUT_SECS,
        "--max-filesize",
        MAX_REMOTE_ART_BYTES_ARG,
        // Both flags keep proxy environment variables from changing the validated destination
        "--noproxy",
        "*",
        "--proxy",
        "",
        "--output",
        "-",
    ]);
    if target.needs_resolve_override {
        command.arg("--resolve").arg(resolve_argument(target));
    }
    command.arg("--url").arg(target.url.as_str());
    command
}

fn resolve_argument(target: &PinnedRemoteTarget) -> String {
    // Curl expects IPv6 values in brackets inside its host-port-address tuple
    let addresses = target
        .addresses
        .iter()
        .map(|address| match address {
            IpAddr::V4(address) => address.to_string(),
            IpAddr::V6(address) => format!("[{address}]"),
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{}:{}:{addresses}", target.host, target.port)
}

fn stop_child(child: &mut std::process::Child) {
    // Always reap rejected transfers so no background process or zombie remains
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
#[path = "tests/remote_art.rs"]
mod tests;
