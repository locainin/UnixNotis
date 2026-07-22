pub(super) fn configure_command_test_root() {
    // A prior UI test may have initialized the one-time root before removing its fixture
    let _ = super::exec::set_command_config_dir(std::env::temp_dir());
    let active_root =
        super::exec::builder::command_config_dir().expect("resolve command test root");
    std::fs::create_dir_all(active_root).expect("create command test root");
}
