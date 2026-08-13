//! Inhibitor mutation and signal fanout helpers for `ControlServer`
//!
//! Keeps inhibit/uninhibit flow and best-effort post-commit fanout isolated

use zbus::message::Header;

use super::{sanitize, ControlServer, MAX_ACTIVE_INHIBITORS};

impl ControlServer {
    pub(super) async fn apply_inhibit(
        &self,
        reason: &str,
        scope: u32,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<u64> {
        self.authorize_control_call(header, "Inhibit").await?;
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::Failed("missing sender".to_string()))?;
        // Clean caller input first
        let normalized_scope = sanitize::normalize_inhibit_scope(scope)?;
        let sanitized_reason = sanitize::sanitize_inhibit_reason(reason);
        // Track inhibitors by unique bus name so cleanup on disconnect is reliable
        let id = {
            let mut store = self.state.store.lock().await;
            if store.inhibitor_count() >= MAX_ACTIVE_INHIBITORS {
                // Hard cap blocks unbounded growth from accidental loops or hostile callers
                return Err(zbus::fdo::Error::Failed(format!(
                    "inhibitor limit reached ({MAX_ACTIVE_INHIBITORS})"
                )));
            }
            store.add_inhibitor(sender.to_string(), sanitized_reason, normalized_scope)
        };
        self.state.publish_inhibitors_changed("added").await;
        Ok(id)
    }

    pub(super) async fn apply_uninhibit(
        &self,
        id: u64,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<()> {
        // Uninhibit trusts ownership on the bus sender, not executable allowlists
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::Failed("missing sender".to_string()))?;
        let owner = sender.to_string();
        // Only the owner can remove it
        let removed = {
            let mut store = self.state.store.lock().await;
            match store.remove_inhibitor(id, &owner) {
                Ok(removed) => removed,
                Err(err) => {
                    return Err(zbus::fdo::Error::AccessDenied(err.message()));
                }
            }
        };
        if !removed {
            // Unknown IDs are treated as a no-op to keep clients resilient
            return Ok(());
        }
        self.state.publish_inhibitors_changed("removed").await;
        Ok(())
    }
}
