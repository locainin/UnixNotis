use clap::Parser;

use super::{run_with_builder, trial_requested};
use crate::cli::Args;
use unixnotis_core::Config;
use zbus::connection::Builder;

#[path = "dbus_lifecycle.rs"]
mod dbus_lifecycle;

#[test]
fn trial_preparation_is_enabled_only_by_the_trial_flag() {
    let normal = Args::try_parse_from(["unixnotis-daemon"]).expect("parse normal daemon command");
    let trial = Args::try_parse_from(["unixnotis-daemon", "--trial", "--yes"])
        .expect("parse trial daemon command");

    assert!(!trial_requested(&normal));
    assert!(trial_requested(&trial));
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_reports_an_unreachable_session_bus() {
    let args = Args::try_parse_from(["unixnotis-daemon"]).expect("parse normal daemon command");
    let builder = Builder::address("unix:path=/nonexistent/unixnotis-test-session-bus")
        .expect("valid unreachable bus address");

    let error = Box::pin(run_with_builder(&args, Config::default(), builder))
        .await
        .expect_err("unreachable session bus should reject startup");

    assert!(
        error.to_string().contains("connect to session bus"),
        "unexpected error: {error:#}"
    );
}
