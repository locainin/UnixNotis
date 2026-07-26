use super::super::env::{test_env_lock, EnvGuard};

#[test]
fn environment_guard_restores_the_original_value() {
    const NAME: &str = "UNIXNOTIS_INSTALLER_ENV_GUARD_TEST";
    let _lock = test_env_lock();
    std::env::set_var(NAME, "before");

    {
        let _guard = EnvGuard::set(NAME, "during");
        assert_eq!(std::env::var_os(NAME).as_deref(), Some("during".as_ref()));
    }

    assert_eq!(std::env::var_os(NAME).as_deref(), Some("before".as_ref()));
    std::env::remove_var(NAME);
}
