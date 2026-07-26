use zbus::Message;

#[cfg(target_os = "linux")]
use super::authorization::required_linux_process_fd;
use super::authorization::{
    authorize_control_call, authorize_panel_readiness_call, authorize_popup_readiness_call,
    control_executable_error, control_owner_uid_error,
};
#[cfg(target_os = "linux")]
use super::credentials::CallerCredentials;
use super::executable_trust::paths::canonicalize_best_effort;
use super::support::write_executable;
use crate::test_support::{daemon_state_for_test, env_lock, EnvVarGuard, TempRoot};

fn message_without_bus_sender() -> Message {
    // Locally built messages have no unique bus sender, which must fail auth early
    Message::method("/", "Ping")
        .expect("method call builder")
        .build(&())
        .expect("method call message")
}

#[tokio::test]
async fn control_authorization_rejects_header_without_bus_sender() {
    let state = daemon_state_for_test(false).await;
    let message = message_without_bus_sender();
    let header = message.header();

    let err = authorize_control_call(&state, &header, "TestControl")
        .await
        .expect_err("missing sender must be rejected");

    assert!(err.to_string().contains("missing sender"));
}

#[tokio::test]
async fn panel_readiness_authorization_rejects_header_without_bus_sender() {
    let state = daemon_state_for_test(false).await;
    let message = message_without_bus_sender();
    let header = message.header();

    let err = authorize_panel_readiness_call(&state, &header, "PanelReady")
        .await
        .expect_err("missing sender must be rejected");

    assert!(err.to_string().contains("missing sender"));
}

#[tokio::test]
async fn popup_readiness_authorization_rejects_header_without_bus_sender() {
    let state = daemon_state_for_test(false).await;
    let message = message_without_bus_sender();
    let header = message.header();

    let err = authorize_popup_readiness_call(&state, &header, "PopupsReady")
        .await
        .expect_err("missing sender must be rejected");

    assert!(err.to_string().contains("missing sender"));
}

#[test]
fn control_uid_error_is_none_only_for_matching_uid() {
    assert!(control_owner_uid_error(1000, 1000).is_none());

    let err = control_owner_uid_error(1001, 1000).expect("mismatched uid should fail");

    assert!(err.to_string().contains("caller uid"));
}

#[test]
fn control_executable_error_requires_present_allowed_trusted_binary() {
    let _guard = env_lock();
    let home = TempRoot::new("auth-executable-error");
    let trusted = home.join(".local/bin/noticenterctl");
    let untrusted_name = home.join(".local/bin/unknown");
    write_executable(&trusted);
    write_executable(&untrusted_name);
    let _home = EnvVarGuard::set("HOME", home.path());
    let trusted = canonicalize_best_effort(&trusted);
    let untrusted_name = canonicalize_best_effort(&untrusted_name);

    assert!(control_executable_error(Some(&trusted), &["noticenterctl"], true).is_none());
    assert!(control_executable_error(None, &["noticenterctl"], true).is_some());
    assert!(control_executable_error(Some(&trusted), &["unixnotis-center"], true).is_some());
    assert!(control_executable_error(Some(&untrusted_name), &["unknown"], true).is_some());
}

#[cfg(target_os = "linux")]
#[test]
fn linux_authorization_rejects_credentials_without_a_stable_process_handle() {
    let credentials = CallerCredentials::default();

    let error = required_linux_process_fd(&credentials)
        .expect_err("Linux authorization must fail without ProcessFD");

    assert!(error.to_string().contains("stable process handle"));
}
