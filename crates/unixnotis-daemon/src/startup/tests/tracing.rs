use crate::test_support::{env_lock, EnvVarGuard};

use crate::startup::tracing::init_tracing;

#[test]
fn init_tracing_installs_a_global_dispatcher() {
    let _guard = env_lock();
    let _rust_log = EnvVarGuard::set("RUST_LOG", "warn");
    let config = unixnotis_core::Config::default();

    let outcome = init_tracing(&config);

    // The daemon should always leave a dispatcher available for later logs
    assert!(outcome.attempted_init);
    assert!(!outcome.had_env_warning);
    assert!(tracing::dispatcher::has_been_set());
}

#[test]
fn init_tracing_reports_invalid_rust_log_warning_path() {
    let _guard = env_lock();
    let _rust_log = EnvVarGuard::set("RUST_LOG", "unixnotis_daemon=definitely-not-a-level");
    let config = unixnotis_core::Config::default();

    let outcome = init_tracing(&config);

    assert!(outcome.attempted_init);
    assert!(outcome.had_env_warning);
}
