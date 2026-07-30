//! Construction and process-bound identity for one MPRIS player

use tokio::sync::watch;
use unixnotis_core::MediaConfig;
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::{Connection, Proxy, ProxyBuilder};

use super::admission::{detect_browser_family, local_art_allowed, remote_art_allowed};
use super::constants::{MPRIS_APP, MPRIS_PATH, MPRIS_PLAYER};

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
    let Some((unique_owner, owner_pid, owner_executable)) =
        resolve_player_owner(connection, name).await
    else {
        // Ownership changed during probing, so a later bus event should rebuild stable data
        return Ok(None);
    };
    // Every process-bound proxy targets the verified unique owner instead of the mutable alias
    let identity = fetch_identity(connection, &unique_owner)
        .await
        .unwrap_or_else(|| name.to_string());
    let browser_family = detect_browser_family(&identity, name, &config.browser_tokens);
    let remote_art_allowed = remote_art_allowed(
        browser_family.as_deref(),
        owner_executable.as_deref(),
        config.remote_art_policy,
    );
    let local_art_allowed = local_art_allowed(
        browser_family.as_deref(),
        owner_executable.as_deref(),
    );
    let player = ProxyBuilder::new(connection)
        .destination(unique_owner.clone())?
        .path(MPRIS_PATH)?
        .interface(MPRIS_PLAYER)?
        .build()
        .await?;
    let properties = PropertiesProxy::builder(connection)
        .destination(unique_owner.clone())?
        .path(MPRIS_PATH)?
        .build()
        .await?;
    let (listener_cancel, _listener_rx) = watch::channel(false);

    Ok(Some(PlayerState {
        bus_name: name.to_string(),
        unique_owner: Some(unique_owner),
        identity,
        browser_family,
        owner_pid,
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
) -> Option<(String, Option<u32>, Option<String>)> {
    // Synthetic names cannot always be converted into a D-Bus bus name
    let Ok(bus_name) = zbus::names::BusName::try_from(name) else {
        return None;
    };
    let Ok(proxy) = DBusProxy::new(connection).await else {
        return None;
    };
    let unique_owner = proxy.get_name_owner(bus_name.clone()).await.ok()?;
    // The bus owner PID is useful for normal players and art trust policy
    // It is weaker than bridge metadata when a helper owns the MPRIS name
    let pid = proxy
        .get_connection_unix_process_id((&unique_owner).into())
        .await
        .ok();
    #[cfg(target_os = "linux")]
    let executable = match pid {
        Some(pid) => read_process_executable_path(pid)
            .await
            .map(|path| path.display().to_string()),
        None => None,
    };
    #[cfg(not(target_os = "linux"))]
    let executable = None;
    let observed_owner = proxy.get_name_owner(bus_name).await.ok()?;
    if !owner_probe_is_stable(unique_owner.as_str(), observed_owner.as_str()) {
        return None;
    }
    Some((unique_owner.to_string(), pid, executable))
}

pub(super) fn owner_probe_is_stable(initial_owner: &str, observed_owner: &str) -> bool {
    initial_owner == observed_owner
}

#[cfg(target_os = "linux")]
async fn read_process_executable_path(pid: u32) -> Option<std::path::PathBuf> {
    // Reading procfs keeps the trust hint tied to the real bus owner process
    tokio::fs::read_link(format!("/proc/{pid}/exe")).await.ok()
}
