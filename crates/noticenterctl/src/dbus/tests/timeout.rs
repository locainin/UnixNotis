use std::time::Duration;

use super::super::timeout::run_control_call_with_timeout;

#[tokio::test]
async fn run_control_call_with_timeout_returns_ready_value() {
    let value =
        run_control_call_with_timeout(Duration::from_secs(1), async { Ok::<_, zbus::Error>(7_u8) })
            .await
            .expect("ready call should succeed");

    assert_eq!(value, 7);
}

#[tokio::test]
async fn run_control_call_with_timeout_reports_expired_call_quickly() {
    let result = run_control_call_with_timeout(Duration::from_millis(1), async {
        tokio::time::sleep(Duration::from_mins(1)).await;
        Ok::<_, zbus::Error>(())
    })
    .await;

    let error = result.expect_err("slow call should time out");
    assert!(error
        .to_string()
        .contains("timed out waiting for unixnotis daemon response"));
}
