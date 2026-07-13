use anyhow::{anyhow, Result};
use std::env;

use super::journal::{
    daemon_unit_from_env, follow_user_unit_logs, journal_has_user_unit_logs,
    journalctl_is_available,
};

pub fn follow_debug_logs() -> Result<()> {
    if !journalctl_is_available() {
        return Err(anyhow!(
            "journalctl is not available; run unixnotis-daemon in a terminal to watch logs directly"
        ));
    }
    let unit = daemon_unit_from_env(|key| env::var(key));
    if !journal_has_user_unit_logs(&unit)? {
        return Err(anyhow!(
            "no user journal stream for {unit}; debug panel open will continue without log follow"
        ));
    }

    follow_user_unit_logs(&unit)
}
