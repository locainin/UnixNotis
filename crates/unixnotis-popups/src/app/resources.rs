//! Process-wide registration for bundled popup resources

use anyhow::Context;

pub fn register() -> anyhow::Result<()> {
    // Registration happens before GTK activation so every card sees the same icon assets
    gio::resources_register_include!("unixnotis-popups.gresource")
        .context("register bundled popup resources")
}
