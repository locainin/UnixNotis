use crate::test_support::{env_lock, EnvVarGuard};

use super::init_tracing;

#[test]
fn init_tracing_installs_a_global_dispatcher() {
    let _guard = env_lock();
    let _rust_log = EnvVarGuard::set("RUST_LOG", "warn");
    let config = unixnotis_core::Config::default();

    init_tracing(&config);

    // The daemon should always leave a dispatcher available for later logs
    assert!(tracing::dispatcher::has_been_set());
}
