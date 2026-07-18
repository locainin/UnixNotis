use super::super::state::MediaRuntimeState;

#[test]
fn new_runtime_state_starts_without_players_cache_or_delayed_work() {
    let state = MediaRuntimeState::new();

    assert!(state.players.is_empty());
    assert!(state.cache.is_empty());
    assert!(state.last_snapshot.is_empty());
    assert!(state.delayed_refreshes.is_empty());
}
