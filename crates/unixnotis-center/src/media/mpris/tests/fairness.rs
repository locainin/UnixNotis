use std::collections::{HashMap, HashSet};
use std::time::Duration;

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::super::constants::MAX_MPRIS_PLAYERS;
use super::super::discovery::{refresh_players, refresh_players_with_builder, DiscoveryState};
use super::super::fairness::MprisFairnessState;
use super::super::inventory::PlayerStateBuildFuture;
use super::super::player::OwnerProbe;
use super::super::PlayerState;
use super::support::{fleet_player_name, MprisFleetFixture};

#[tokio::test]
async fn full_capacity_fairness_becomes_due_at_its_monotonic_deadline() {
    let lease = Duration::from_millis(20);
    let mut fairness = MprisFairnessState::with_durations(lease, lease);
    let (signal_tx, mut signal_rx) = mpsc::channel(1);
    let admitted_at = tokio::time::Instant::now();

    assert!(!fairness.rotation_due(true, true, admitted_at, &signal_tx));
    receive_fairness_wakeup(&mut fairness, &mut signal_rx).await;
    assert!(fairness.rotation_due(true, true, tokio::time::Instant::now(), &signal_tx));
}

#[tokio::test]
async fn fairness_never_schedules_below_capacity_or_without_untracked_candidates() {
    let lease = Duration::from_millis(10);
    let mut fairness = MprisFairnessState::with_durations(lease, lease);
    let (signal_tx, mut signal_rx) = mpsc::channel(1);

    assert!(!fairness.rotation_due(false, true, tokio::time::Instant::now(), &signal_tx));
    assert!(!fairness.rotation_due(true, false, tokio::time::Instant::now(), &signal_tx));
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(signal_rx.try_recv().is_err());
}

#[test]
fn fairness_victim_selection_rotates_across_incumbents() {
    let mut fairness = MprisFairnessState::new();
    let tracked = HashSet::from([
        "org.mpris.MediaPlayer2.a".to_string(),
        "org.mpris.MediaPlayer2.b".to_string(),
    ]);

    assert_eq!(
        fairness.select_victim(&tracked).as_deref(),
        Some("org.mpris.MediaPlayer2.a")
    );
    assert_eq!(
        fairness.select_victim(&tracked).as_deref(),
        Some("org.mpris.MediaPlayer2.b")
    );
}

async fn receive_fairness_wakeup(
    fairness: &mut MprisFairnessState,
    signal_rx: &mut mpsc::Receiver<crate::media::runtime::MediaSignal>,
) {
    let signal = tokio::time::timeout(Duration::from_secs(2), signal_rx.recv())
        .await
        .expect("quiet capacity should receive its fairness wakeup")
        .expect("fairness signal channel should remain open");
    let crate::media::runtime::MediaSignal::FairnessLeaseExpired { generation } = signal else {
        panic!("quiet MPRIS players emitted an unrelated signal");
    };
    assert!(fairness.consume_wakeup(generation));
}

fn cancel_all_listeners(players: &HashMap<String, PlayerState>) {
    for player in players.values() {
        let _ = player.listener_cancel.send(true);
    }
}

async fn discover_incumbent_fleet(
    fixture: &MprisFleetFixture,
    proxy: &DBusProxy<'_>,
    config: &MediaConfig,
    signal_tx: &mpsc::Sender<crate::media::runtime::MediaSignal>,
    players: &mut HashMap<String, PlayerState>,
    cursor: &mut usize,
    fairness: &mut MprisFairnessState,
) {
    refresh_players(
        &fixture.client,
        proxy,
        config,
        signal_tx,
        players,
        cursor,
        fairness,
    )
    .await
    .expect("discover the incumbent fleet");
    assert_eq!(players.len(), MAX_MPRIS_PLAYERS);
}

#[tokio::test]
async fn quiet_full_capacity_inventory_wakes_and_admits_the_next_player() {
    let mut fixture = MprisFleetFixture::start(MAX_MPRIS_PLAYERS).await;
    let config = MediaConfig::default();
    let proxy = DBusProxy::new(&fixture.client)
        .await
        .expect("create private bus proxy");
    let (signal_tx, mut signal_rx) = mpsc::channel(64);
    let mut players = HashMap::new();
    let mut cursor = 0;
    let mut fairness =
        MprisFairnessState::with_durations(Duration::from_millis(25), Duration::from_millis(25));
    discover_incumbent_fleet(
        &fixture,
        &proxy,
        &config,
        &signal_tx,
        &mut players,
        &mut cursor,
        &mut fairness,
    )
    .await;

    let candidate_name = fleet_player_name(MAX_MPRIS_PLAYERS);
    fixture.add_player(MAX_MPRIS_PLAYERS).await;
    refresh_players(
        &fixture.client,
        &proxy,
        &config,
        &signal_tx,
        &mut players,
        &mut cursor,
        &mut fairness,
    )
    .await
    .expect("observe the over-capacity candidate");
    assert!(!players.contains_key(&candidate_name));

    // The lease task is the only event that requests this second discovery pass
    receive_fairness_wakeup(&mut fairness, &mut signal_rx).await;
    refresh_players(
        &fixture.client,
        &proxy,
        &config,
        &signal_tx,
        &mut players,
        &mut cursor,
        &mut fairness,
    )
    .await
    .expect("admit the fairness candidate after its deadline");

    assert_eq!(players.len(), MAX_MPRIS_PLAYERS);
    assert!(players.contains_key(&candidate_name));
    cancel_all_listeners(&players);
}

fn fail_player_state_build<'a>(
    _connection: &'a Connection,
    _name: &'a str,
    _config: &'a MediaConfig,
    _owner: OwnerProbe,
) -> PlayerStateBuildFuture<'a> {
    Box::pin(async {
        Err(zbus::Error::Failure(
            "intentional candidate build failure".to_string(),
        ))
    })
}

#[tokio::test]
async fn failed_fairness_candidate_build_keeps_the_incumbent_and_listener_alive() {
    let mut fixture = MprisFleetFixture::start(MAX_MPRIS_PLAYERS).await;
    let config = MediaConfig::default();
    let proxy = DBusProxy::new(&fixture.client)
        .await
        .expect("create private bus proxy");
    let (signal_tx, mut signal_rx) = mpsc::channel(64);
    let mut players = HashMap::new();
    let mut cursor = 0;
    let mut fairness =
        MprisFairnessState::with_durations(Duration::from_millis(25), Duration::from_millis(100));
    discover_incumbent_fleet(
        &fixture,
        &proxy,
        &config,
        &signal_tx,
        &mut players,
        &mut cursor,
        &mut fairness,
    )
    .await;
    fixture.add_player(MAX_MPRIS_PLAYERS).await;
    refresh_players(
        &fixture.client,
        &proxy,
        &config,
        &signal_tx,
        &mut players,
        &mut cursor,
        &mut fairness,
    )
    .await
    .expect("start the fairness lease");
    receive_fairness_wakeup(&mut fairness, &mut signal_rx).await;
    let victim_name = fleet_player_name(0);
    let mut victim_cancel = players[&victim_name].listener_cancel.subscribe();

    refresh_players_with_builder(
        &fixture.client,
        &proxy,
        &config,
        &signal_tx,
        DiscoveryState {
            players: &mut players,
            discovery_cursor: &mut cursor,
            fairness: &mut fairness,
        },
        fail_player_state_build,
    )
    .await
    .expect("candidate build failure should not fail discovery");

    assert_eq!(players.len(), MAX_MPRIS_PLAYERS);
    assert!(players.contains_key(&victim_name));
    assert!(!players.contains_key(&fleet_player_name(MAX_MPRIS_PLAYERS)));
    assert!(
        tokio::time::timeout(Duration::from_millis(30), victim_cancel.changed())
            .await
            .is_err(),
        "failed admission must not cancel the selected incumbent"
    );
    cancel_all_listeners(&players);
}
