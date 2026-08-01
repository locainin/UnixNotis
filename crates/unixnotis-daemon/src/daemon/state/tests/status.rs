use crate::test_support::{daemon_state_for_test, daemon_state_for_test_with_owner};

#[tokio::test]
async fn daemon_state_boolean_flags_reflect_runtime_updates() {
    let state = daemon_state_for_test(true).await;

    assert!(state.trial_mode());
    assert!(!state.panel_ready());
    assert!(!state.popups_process_running());

    // These health flags gate user-visible command handling, so getters must reflect writes exactly
    state.set_center_process_running(true);
    state.set_panel_ready(":1.20", true);
    state.set_popups_process_running(true);

    assert!(state.panel_ready());
    assert!(state.popups_process_running());
    state.set_popups_ready(":1.10", true);

    let health = state.ui_health();
    assert!(health.center_process_running);
    assert!(health.center_ready);
    assert!(health.popups_process_running);
    assert!(health.popups_ready);
}

#[tokio::test]
async fn daemon_state_boolean_flags_can_return_to_false() {
    let state = daemon_state_for_test(true).await;

    state.set_panel_ready(":1.20", true);
    state.set_center_process_running(true);
    state.set_popups_process_running(true);
    state.set_panel_ready(":1.20", false);
    state.set_center_process_running(false);
    state.set_popups_process_running(false);

    assert!(!state.panel_ready());
    assert!(!state.popups_process_running());
}

#[tokio::test]
async fn popup_readiness_can_only_be_cleared_by_its_owner_generation() {
    let state = daemon_state_for_test(true).await;
    state.set_popups_process_running(true);
    state.set_popups_ready(":1.10", true);

    state.set_popups_ready(":1.11", false);
    assert!(state.popups_ready());

    state.set_popups_ready(":1.10", false);
    assert!(!state.popups_ready());
}

#[tokio::test]
async fn panel_owner_loss_clears_readiness_and_panel_availability() {
    let state = daemon_state_for_test(true).await;
    state.set_center_process_running(true);
    state.set_panel_ready(":1.20", true);
    assert!(state.panel_ready());

    state.remove_disconnected_client(":1.20").await;

    assert!(!state.panel_ready());
}

#[tokio::test]
async fn delayed_old_panel_disconnect_keeps_new_owner_ready() {
    let state = daemon_state_for_test(true).await;
    state.set_center_process_running(true);
    state.set_panel_ready(":1.20", true);
    state.set_panel_ready(":1.21", true);

    state.remove_disconnected_client(":1.20").await;

    assert!(state.panel_ready());
    assert!(state.ui_health().center_ready);
    state.remove_disconnected_client(":1.21").await;
    assert!(!state.panel_ready());
}

#[tokio::test]
async fn popup_owner_loss_clears_readiness_for_the_matching_generation() {
    let state = daemon_state_for_test(true).await;
    state.set_popups_process_running(true);
    state.set_popups_ready(":1.10", true);

    state.remove_disconnected_client(":1.10").await;

    assert!(!state.popups_ready());
}

#[tokio::test]
async fn stopped_popup_process_clears_composite_readiness() {
    let state = daemon_state_for_test(true).await;
    state.set_popups_process_running(true);
    state.set_popups_ready(":1.10", true);

    state.set_popups_process_running(false);

    let health = state.ui_health();
    assert!(!health.popups_process_running);
    assert!(!health.popups_ready);
}

#[tokio::test]
async fn daemon_state_trial_mode_can_be_disabled() {
    let state = daemon_state_for_test(false).await;

    // Trial mode changes control authorization, so false must stay observable
    assert!(!state.trial_mode());
}

#[tokio::test]
async fn popup_unready_warning_is_emitted_only_once_until_ready() {
    let state = daemon_state_for_test(true).await;

    assert!(state.should_warn_popups_unready());
    assert!(!state.should_warn_popups_unready());

    state.set_popups_process_running(true);
    state.set_popups_ready(":1.10", true);
    assert!(!state.should_warn_popups_unready());
    state.set_popups_ready(":1.10", false);

    assert!(state.should_warn_popups_unready());
}

#[tokio::test]
async fn control_owner_preauthorization_matches_only_the_current_owner() {
    let state = daemon_state_for_test_with_owner(true, Some(":1.42")).await;

    assert!(state.control_owner_is_preauthorized(":1.42"));
    assert!(!state.control_owner_is_preauthorized(":1.43"));
}
