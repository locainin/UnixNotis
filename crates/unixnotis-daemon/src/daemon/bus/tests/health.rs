use std::io;
use std::sync::Arc;

use super::{
    definitive_transport_failure, probe_error_outcome, BusProbeOutcome, TransientFailureCounter,
    MAX_CONSECUTIVE_TRANSIENT_FAILURES,
};

#[test]
fn one_transient_probe_timeout_keeps_monitor_policy_alive() {
    let mut failures = TransientFailureCounter::default();

    assert!(!failures.observe_failure());
    assert_eq!(failures.consecutive, 1);
}

#[test]
fn two_transient_failures_then_success_reset_the_failure_counter() {
    let mut failures = TransientFailureCounter::default();

    assert!(!failures.observe_failure());
    assert!(!failures.observe_failure());
    failures.observe_healthy();

    assert_eq!(failures.consecutive, 0);
    assert!(!failures.observe_failure());
}

#[test]
fn repeated_transient_failures_become_fatal_at_the_configured_limit() {
    let mut failures = TransientFailureCounter::default();

    for _ in 1..MAX_CONSECUTIVE_TRANSIENT_FAILURES {
        assert!(!failures.observe_failure());
    }

    assert!(failures.observe_failure());
}

#[test]
fn name_without_an_owner_is_a_definitive_loss() {
    let outcome = BusProbeOutcome::DefinitiveNameLoss {
        name: unixnotis_core::CONTROL_BUS_NAME,
        owner: None,
    };

    assert!(matches!(
        outcome,
        BusProbeOutcome::DefinitiveNameLoss { owner: None, .. }
    ));
}

#[test]
fn different_owner_is_a_definitive_loss() {
    let outcome = BusProbeOutcome::DefinitiveNameLoss {
        name: unixnotis_core::NOTIFICATIONS_BUS_NAME,
        owner: Some(":1.99".to_string()),
    };

    assert!(matches!(
        outcome,
        BusProbeOutcome::DefinitiveNameLoss {
            owner: Some(owner),
            ..
        } if owner == ":1.99"
    ));
}

#[test]
fn concrete_closed_socket_errors_are_definitive_transport_failures() {
    let disconnected = zbus::fdo::Error::Disconnected("closed test connection".to_string());
    let closed = zbus::fdo::Error::ZBus(zbus::Error::InputOutput(Arc::new(io::Error::new(
        io::ErrorKind::BrokenPipe,
        "closed test socket",
    ))));
    let interrupted = zbus::fdo::Error::ZBus(zbus::Error::InputOutput(Arc::new(io::Error::new(
        io::ErrorKind::Interrupted,
        "interrupted test operation",
    ))));

    assert!(definitive_transport_failure(&disconnected));
    assert!(definitive_transport_failure(&closed));
    assert!(!definitive_transport_failure(&interrupted));
}

#[test]
fn probe_error_dispatch_distinguishes_name_loss_transport_loss_and_transient_failure() {
    let name_loss = probe_error_outcome(
        unixnotis_core::CONTROL_BUS_NAME,
        zbus::fdo::Error::NameHasNoOwner("missing test owner".to_string()),
    );
    let transport_loss = probe_error_outcome(
        unixnotis_core::CONTROL_BUS_NAME,
        zbus::fdo::Error::Disconnected("closed test connection".to_string()),
    );
    let transient = probe_error_outcome(
        unixnotis_core::CONTROL_BUS_NAME,
        zbus::fdo::Error::NoReply("temporary test timeout".to_string()),
    );

    assert!(matches!(
        name_loss,
        BusProbeOutcome::DefinitiveNameLoss { owner: None, .. }
    ));
    assert!(matches!(
        transport_loss,
        BusProbeOutcome::DefinitiveTransportFailure(_)
    ));
    assert!(matches!(transient, BusProbeOutcome::TransientFailure(_)));
}
