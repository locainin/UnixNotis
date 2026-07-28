//! Process-wide registration for bundled popup resources

use std::sync::OnceLock;

pub fn register() -> anyhow::Result<()> {
    static REGISTRATION: OnceLock<Result<(), String>> = OnceLock::new();
    // Registration happens before GTK activation so every card sees the same icon assets
    match REGISTRATION.get_or_init(|| {
        gio::resources_register_include!("unixnotis-popups.gresource")
            .map_err(|error| format!("register bundled popup resources: {error}"))
    }) {
        Ok(()) => Ok(()),
        Err(error) => Err(anyhow::anyhow!(error.clone())),
    }
}
