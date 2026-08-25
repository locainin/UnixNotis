use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::anyhow;

use super::{stable_user_bus_address, wait_until_ready_with_probe};

#[tokio::test]
async fn installer_rejects_process_start_success_without_dbus_readiness() {
    let status = std::process::Command::new("true")
        .status()
        .expect("fake service start command should run");
    assert!(status.success());

    let error = wait_until_ready_with_probe(Duration::from_millis(15), || {
        std::future::ready(Err(anyhow!("both required names have no owner")))
    })
    .await
    .expect_err("a successful process start must not satisfy D-Bus readiness");

    assert!(error.to_string().contains("readiness timed out"));
    assert!(error.to_string().contains("both required names"));
}

#[tokio::test]
async fn readiness_gate_returns_after_the_first_complete_probe() {
    let probes = AtomicUsize::new(0);

    wait_until_ready_with_probe(Duration::from_secs(1), || {
        probes.fetch_add(1, Ordering::Relaxed);
        std::future::ready(Ok(()))
    })
    .await
    .expect("complete readiness should pass");

    assert_eq!(probes.load(Ordering::Relaxed), 1);
}

#[test]
fn stable_bus_address_uses_the_current_numeric_uid() {
    assert_eq!(
        stable_user_bus_address(),
        format!(
            "unix:path=/run/user/{}/bus",
            rustix::process::getuid().as_raw()
        )
    );
}
