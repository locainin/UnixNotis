use clap::Parser;

use std::process::Command;

use super::{run, run_with_builder, trial_requested};
use crate::cli::Args;
use unixnotis_core::Config;
use zbus::connection::Builder;

#[path = "dbus_lifecycle.rs"]
mod dbus_lifecycle;

const RUNTIME_CHILD_ENV: &str = "UNIXNOTIS_RUNTIME_TEST_CHILD";
const RUNTIME_ERROR_TEST: &str =
    "runtime::runner::tests::public_runtime_returns_error_when_session_bus_is_unreachable";

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

#[test]
fn public_runtime_returns_error_when_session_bus_is_unreachable() {
    if std::env::var_os(RUNTIME_CHILD_ENV).is_some() {
        // The child owns its environment, so no parallel test can observe the fake bus address
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build isolated runtime test executor");
        let args = Args::try_parse_from(["unixnotis-daemon"]).expect("parse normal daemon command");
        let error = runtime
            .block_on(Box::pin(run(&args, Config::default())))
            .expect_err("public runtime must propagate an unreachable session bus");
        assert!(
            error.to_string().contains("session bus"),
            "unexpected public runtime error: {error:#}"
        );
        return;
    }

    // A child process scopes the D-Bus environment mutation to this one regression
    let test_binary = std::env::current_exe().expect("resolve current daemon test binary");
    let status = Command::new(test_binary)
        .args(["--exact", RUNTIME_ERROR_TEST, "--nocapture"])
        .env(RUNTIME_CHILD_ENV, "1")
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/unixnotis-public-runtime-session-bus",
        )
        .env_remove("DBUS_STARTER_ADDRESS")
        .status()
        .expect("run isolated public runtime regression");

    assert!(
        status.success(),
        "isolated public runtime regression must pass"
    );
}
