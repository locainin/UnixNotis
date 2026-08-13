use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use zbus::zvariant::OwnedValue;
use zbus::{Connection, ConnectionBuilder};

use super::super::constants::MPRIS_PATH;
use super::super::player::{build_player_state_for_owner, resolve_player_owner, PlayerState};
use crate::test_support::broker::read_broker_address;
use unixnotis_core::MediaConfig;

pub(in crate::media) const TEST_PLAYER_NAME: &str = "org.mpris.MediaPlayer2.unixnotis_test";
pub(in crate::media) const TEST_BRIDGE_PLAYER_NAME: &str =
    "org.mpris.MediaPlayer2.plasma-browser-integration";
pub(in crate::media) const TEST_PLAYER_IDENTITY: &str = "UnixNotis Test Player";

pub(in crate::media) async fn build_player_state(
    connection: &Connection,
    name: &str,
    config: &MediaConfig,
) -> zbus::Result<Option<PlayerState>> {
    // Resolve one stable owner before constructing proxies bound to that owner
    let Some(owner) = resolve_player_owner(connection, name).await else {
        return Ok(None);
    };
    Ok(Some(
        build_player_state_for_owner(connection, name, config, owner).await?,
    ))
}

pub(in crate::media) fn fleet_player_name(index: usize) -> String {
    format!("org.mpris.MediaPlayer2.unixnotis_fleet_{index:03}")
}

// Parallel fixtures need distinct socket directories even inside one process
static NEXT_BROKER: AtomicUsize = AtomicUsize::new(0);

struct PrivateBroker {
    child: Child,
    socket: PathBuf,
    address: String,
}

impl PrivateBroker {
    fn start() -> Self {
        let socket = broker_socket();
        let listen_address = format!("unix:path={}", socket.display());
        // Fixed system roots keep the fixture independent from mutable shell search paths
        let daemon = unixnotis_core::util::trusted_system_program_path("dbus-daemon")
            .expect("find dbus-daemon in a trusted system directory");
        let mut child = Command::new(daemon)
            .args([
                "--session",
                "--nofork",
                "--nopidfile",
                "--print-address=1",
                &format!("--address={listen_address}"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start private D-Bus broker");
        // The first output line is the exact address accepted by the broker
        let stdout = child.stdout.take().expect("capture broker address");
        let address = read_broker_address(&mut child, stdout, &listen_address)
            .expect("read private broker address promptly");

        Self {
            child,
            socket,
            address,
        }
    }
}

impl Drop for PrivateBroker {
    fn drop(&mut self) {
        // Reaping the broker prevents test processes and socket trees from leaking
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
        if let Some(parent) = self.socket.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

fn broker_socket() -> PathBuf {
    // Time, process, and serial values keep concurrent test roots independent
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after the Unix epoch")
        .as_nanos();
    let serial = NEXT_BROKER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-media-dbus-{}-{stamp}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create private broker directory");
    root.join("bus.sock")
}

struct TestMprisRoot {
    identity: String,
}

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl TestMprisRoot {
    #[zbus(property)]
    fn identity(&self) -> &str {
        // A fixed identity makes player construction assertions deterministic
        &self.identity
    }
}

#[derive(Default)]
struct CommandCounts {
    next: AtomicUsize,
    play_pause: AtomicUsize,
    previous: AtomicUsize,
}

struct TestMprisPlayer {
    commands: Arc<CommandCounts>,
    metadata_bytes: usize,
    art_url_bytes: usize,
    metadata_pid: Option<u32>,
    slow_non_status: bool,
}

async fn delay_non_status_property(slow: bool) {
    if slow {
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;
    }
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl TestMprisPlayer {
    fn next(&self) {
        // Atomic counters prove that commands crossed the real local bus
        self.commands.next.fetch_add(1, Ordering::Relaxed);
    }

    fn play_pause(&self) {
        self.commands.play_pause.fetch_add(1, Ordering::Relaxed);
    }

    fn previous(&self) {
        self.commands.previous.fetch_add(1, Ordering::Relaxed);
    }

    #[zbus(property)]
    async fn metadata(&self) -> HashMap<String, OwnedValue> {
        delay_non_status_property(self.slow_non_status).await;
        // The optional payload exercises the raw reply budget without a real player
        let mut metadata = HashMap::new();
        if self.metadata_bytes > 0 {
            let large_value = "x".repeat(self.metadata_bytes);
            metadata.insert(
                "test:large".to_string(),
                OwnedValue::try_from(zbus::zvariant::Value::from(large_value.as_str()))
                    .expect("build large metadata value"),
            );
        }
        if self.art_url_bytes > 0 {
            let art_url = format!("https://example.com/{}", "x".repeat(self.art_url_bytes));
            metadata.insert(
                "mpris:artUrl".to_string(),
                OwnedValue::try_from(zbus::zvariant::Value::from(art_url.as_str()))
                    .expect("build art URL value"),
            );
        }
        if let Some(pid) = self.metadata_pid {
            metadata.insert("kde:pid".to_string(), OwnedValue::from(pid));
        }
        metadata
    }

    #[zbus(property)]
    fn playback_status(&self) -> &'static str {
        "Playing"
    }

    #[zbus(property)]
    async fn can_play(&self) -> bool {
        delay_non_status_property(self.slow_non_status).await;
        true
    }

    #[zbus(property)]
    async fn can_pause(&self) -> bool {
        delay_non_status_property(self.slow_non_status).await;
        true
    }

    #[zbus(property)]
    async fn can_go_next(&self) -> bool {
        delay_non_status_property(self.slow_non_status).await;
        true
    }

    #[zbus(property)]
    async fn can_go_previous(&self) -> bool {
        delay_non_status_property(self.slow_non_status).await;
        true
    }
}

pub(in crate::media) struct MprisFixture {
    // Both ends stay alive for the full test so unique owner data remains stable
    pub(in crate::media) server: Connection,
    pub(in crate::media) client: Connection,
    commands: Arc<CommandCounts>,
    _broker: PrivateBroker,
}

impl MprisFixture {
    pub(in crate::media) async fn start() -> Self {
        Self::start_with_payload(0, 0, 0).await
    }

    pub(in crate::media) async fn start_with_metadata_bytes(metadata_bytes: usize) -> Self {
        Self::start_with_payload(metadata_bytes, 0, 0).await
    }

    pub(in crate::media) async fn start_with_art_url_bytes(art_url_bytes: usize) -> Self {
        Self::start_with_payload(0, art_url_bytes, 0).await
    }

    pub(in crate::media) async fn start_with_kde_pid(pid: u32) -> Self {
        Self::start_with_payload_for_name(TEST_BRIDGE_PLAYER_NAME, 0, 0, 0, Some(pid), false).await
    }

    pub(in crate::media) async fn start_with_identity_bytes(identity_bytes: usize) -> Self {
        Self::start_with_payload(0, 0, identity_bytes).await
    }

    pub(in crate::media) async fn start_with_slow_non_status_properties() -> Self {
        Self::start_with_payload_for_name(TEST_PLAYER_NAME, 0, 0, 0, None, true).await
    }

    async fn start_with_payload(
        metadata_bytes: usize,
        art_url_bytes: usize,
        identity_bytes: usize,
    ) -> Self {
        Self::start_with_payload_for_name(
            TEST_PLAYER_NAME,
            metadata_bytes,
            art_url_bytes,
            identity_bytes,
            None,
            false,
        )
        .await
    }

    async fn start_with_payload_for_name(
        name: &str,
        metadata_bytes: usize,
        art_url_bytes: usize,
        identity_bytes: usize,
        metadata_pid: Option<u32>,
        slow_non_status: bool,
    ) -> Self {
        let broker = PrivateBroker::start();
        let identity = if identity_bytes == 0 {
            TEST_PLAYER_IDENTITY.to_string()
        } else {
            "x".repeat(identity_bytes)
        };
        let (server, commands) = build_test_player_service(
            &broker.address,
            name,
            identity,
            metadata_bytes,
            art_url_bytes,
            metadata_pid,
            slow_non_status,
        )
        .await;
        // A separate client connection exercises normal bus routing and owner lookup
        let client = ConnectionBuilder::address(broker.address.as_str())
            .expect("parse private broker address")
            .build()
            .await
            .expect("connect test MPRIS client");

        Self {
            server,
            client,
            commands,
            _broker: broker,
        }
    }

    pub(in crate::media) fn next_calls(&self) -> usize {
        self.commands.next.load(Ordering::Relaxed)
    }

    pub(in crate::media) async fn emit_playback_status_changed(&self) {
        // The generated helper sends the same PropertiesChanged signal as a real player
        let interface = self
            .server
            .object_server()
            .interface::<_, TestMprisPlayer>(MPRIS_PATH)
            .await
            .expect("find test MPRIS player interface");
        interface
            .get()
            .await
            .playback_status_changed(interface.signal_context())
            .await
            .expect("emit playback status change");
    }
}

pub(in crate::media) struct MprisFleetFixture {
    pub(in crate::media) client: Connection,
    servers: Vec<Connection>,
    broker: PrivateBroker,
}

impl MprisFleetFixture {
    pub(in crate::media) async fn start(player_count: usize) -> Self {
        let broker = PrivateBroker::start();
        let client = ConnectionBuilder::address(broker.address.as_str())
            .expect("parse private broker address")
            .build()
            .await
            .expect("connect fleet test client");
        let mut fixture = Self {
            client,
            servers: Vec::with_capacity(player_count.saturating_add(1)),
            broker,
        };
        for index in 0..player_count {
            fixture.add_player(index).await;
        }
        fixture
    }

    pub(in crate::media) async fn add_player(&mut self, index: usize) {
        let name = fleet_player_name(index);
        let identity = format!("UnixNotis Fleet Player {index:03}");
        let (server, _commands) =
            build_test_player_service(&self.broker.address, &name, identity, 0, 0, None, false)
                .await;
        // Keeping the connection alive preserves the unique owner and all exported interfaces
        self.servers.push(server);
    }
}

async fn build_test_player_service(
    address: &str,
    name: &str,
    identity: String,
    metadata_bytes: usize,
    art_url_bytes: usize,
    metadata_pid: Option<u32>,
    slow_non_status: bool,
) -> (Connection, Arc<CommandCounts>) {
    let commands = Arc::new(CommandCounts::default());
    // The service exports both MPRIS interfaces at the standard object path
    let server = ConnectionBuilder::address(address)
        .expect("parse private broker address")
        .name(name)
        .expect("request test MPRIS name")
        .serve_at(MPRIS_PATH, TestMprisRoot { identity })
        .expect("register test MPRIS root")
        .serve_at(
            MPRIS_PATH,
            TestMprisPlayer {
                commands: commands.clone(),
                metadata_bytes,
                art_url_bytes,
                metadata_pid,
                slow_non_status,
            },
        )
        .expect("register test MPRIS player")
        .build()
        .await
        .expect("connect test MPRIS service");
    (server, commands)
}
