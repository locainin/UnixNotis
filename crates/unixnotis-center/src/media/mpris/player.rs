//! Construction and process-bound identity for one MPRIS player

use tokio::sync::watch;
use unixnotis_core::MediaConfig;
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::{Connection, Proxy, ProxyBuilder};

use super::admission::{detect_browser_family, local_art_allowed, remote_art_allowed};
use super::constants::{MPRIS_APP, MPRIS_PATH, MPRIS_PLAYER};
#[cfg(target_os = "linux")]
use super::process::executable_allowed_from_pidfd;
#[cfg(target_os = "linux")]
use super::process::read_process_executable_path_from_pidfd;
#[cfg(target_os = "linux")]
use zbus::zvariant::OwnedFd;

#[cfg(target_os = "linux")]
use super::credentials::get_connection_credentials;

#[derive(Clone)]
pub(in crate::media) struct PlayerState {
    pub(in crate::media) bus_name: String,
    // Unique owner distinguishes a restarted process that reused the same MPRIS name
    pub(in crate::media) unique_owner: Option<String>,
    pub(in crate::media) identity: String,
    pub(in crate::media) browser_family: Option<String>,
    pub(in crate::media) owner_pid: Option<u32>,
    pub(in crate::media) remote_art_allowed: bool,
    pub(in crate::media) local_art_allowed: bool,
    pub(in crate::media) player: Proxy<'static>,
    pub(in crate::media) properties: PropertiesProxy<'static>,
    // Cancellation sender for the properties listener task
    pub(in crate::media) listener_cancel: watch::Sender<bool>,
}

pub(in crate::media) async fn build_player_state(
    connection: &Connection,
    name: &str,
    config: &MediaConfig,
) -> zbus::Result<Option<PlayerState>> {
    // D-Bus owner data is captured once so snapshots do not need another bus round trip
    // Browser bridges may later override this PID with a stronger metadata source PID
    let Some(owner) = resolve_player_owner(connection, name).await else {
        // Ownership changed during probing, so a later bus event should rebuild stable data
        return Ok(None);
    };
    // Every process-bound proxy targets the verified unique owner instead of the mutable alias
    let identity = fetch_identity(connection, &owner.unique_owner)
        .await
        .unwrap_or_else(|| name.to_string());
    let browser_family = detect_browser_family(&identity, name, &config.browser_tokens);
    let remote_art_allowed = remote_art_allowed(
        browser_family.as_deref(),
        owner.executable.as_deref(),
        config.remote_art_policy,
    );
    #[cfg(target_os = "linux")]
    let owner_executable_is_allowed = owner.process_fd.as_ref().is_some_and(|process_fd| {
        executable_allowed_from_pidfd(
            process_fd,
            owner.pid,
            &config.local_art_executable_allowlist,
        )
    });
    #[cfg(not(target_os = "linux"))]
    let owner_executable_is_allowed = false;
    let local_art_allowed = local_art_allowed(
        browser_family.as_deref(),
        owner.executable.as_deref(),
        owner_executable_is_allowed,
        config.local_art_policy,
    );
    let player = ProxyBuilder::new(connection)
        .destination(owner.unique_owner.clone())?
        .path(MPRIS_PATH)?
        .interface(MPRIS_PLAYER)?
        .build()
        .await?;
    let properties = PropertiesProxy::builder(connection)
        .destination(owner.unique_owner.clone())?
        .path(MPRIS_PATH)?
        .build()
        .await?;
    let (listener_cancel, _listener_rx) = watch::channel(false);

    Ok(Some(PlayerState {
        bus_name: name.to_string(),
        unique_owner: Some(owner.unique_owner),
        identity,
        browser_family,
        owner_pid: Some(owner.pid),
        remote_art_allowed,
        local_art_allowed,
        player,
        properties,
        listener_cancel,
    }))
}

pub(super) async fn fetch_identity(connection: &Connection, name: &str) -> Option<String> {
    let proxy: Proxy<'static> = ProxyBuilder::new(connection)
        .destination(name.to_string())
        .ok()?
        .path(MPRIS_PATH)
        .ok()?
        .interface(MPRIS_APP)
        .ok()?
        .build()
        .await
        .ok()?;
    proxy.get_property("Identity").await.ok()
}

pub(super) async fn resolve_player_owner(
    connection: &Connection,
    name: &str,
) -> Option<OwnerProbe> {
    // Synthetic names cannot always be converted into a D-Bus bus name
    let Ok(bus_name) = zbus::names::BusName::try_from(name) else {
        return None;
    };
    let Ok(proxy) = DBusProxy::new(connection).await else {
        return None;
    };
    let unique_owner = proxy.get_name_owner(bus_name.clone()).await.ok()?;
    #[cfg(target_os = "linux")]
    let credentials = get_connection_credentials(connection, (&unique_owner).into()).await?;
    #[cfg(target_os = "linux")]
    let (pid, process_fd) = (credentials.process_id?, credentials.process_fd);
    #[cfg(not(target_os = "linux"))]
    let pid = proxy
        .get_connection_unix_process_id((&unique_owner).into())
        .await
        .ok()?;
    let observed_owner = proxy.get_name_owner(bus_name).await.ok()?;
    if !owner_probe_is_stable(unique_owner.as_str(), observed_owner.as_str()) {
        return None;
    }
    #[cfg(target_os = "linux")]
    let executable =
        read_owner_executable_path(pid, process_fd.as_ref()).map(|path| path.display().to_string());
    #[cfg(target_os = "linux")]
    executable.as_ref()?;
    Some(OwnerProbe {
        unique_owner: unique_owner.to_string(),
        pid,
        executable,
        #[cfg(target_os = "linux")]
        process_fd,
    })
}

pub(super) struct OwnerProbe {
    pub(super) unique_owner: String,
    pub(super) pid: u32,
    pub(super) executable: Option<String>,
    #[cfg(target_os = "linux")]
    pub(super) process_fd: Option<OwnedFd>,
}

#[cfg(target_os = "linux")]
pub(super) fn read_owner_executable_path(
    pid: u32,
    process_fd: Option<&OwnedFd>,
) -> Option<std::path::PathBuf> {
    // A ProcessFD gives a stable object; older buses may provide only the PID
    if let Some(process_fd) = process_fd {
        return read_process_executable_path_from_pidfd(process_fd, pid);
    }

    std::fs::read_link(format!("/proc/{pid}/exe")).ok()
}

pub(super) fn owner_probe_is_stable(initial_owner: &str, observed_owner: &str) -> bool {
    initial_owner == observed_owner
}
