//! Inhibitor mutation and signal fanout helpers for ControlServer
//!
//! Keeps inhibit/uninhibit flow and best-effort post-commit fanout isolated

use tracing::warn;
use zbus::message::Header;
use zbus::SignalContext;

use unixnotis_core::CONTROL_OBJECT_PATH;

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
        let (id, active, count) = {
            let mut store = self.state.store.lock().await;
            if store.inhibitor_count() >= MAX_ACTIVE_INHIBITORS {
                // Hard cap blocks unbounded growth from accidental loops or hostile callers
                return Err(zbus::fdo::Error::Failed(format!(
                    "inhibitor limit reached ({MAX_ACTIVE_INHIBITORS})"
                )));
            }
            let id = store.add_inhibitor(sender.to_string(), sanitized_reason, normalized_scope);
            let active = store.inhibited();
            let count = store.inhibitor_count();
            (id, active, count)
        };
        self.emit_inhibitor_updates(active, count, "added").await;
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
        let (removed, active, count) = {
            let mut store = self.state.store.lock().await;
            match store.remove_inhibitor(id, &owner) {
                Ok(removed) => {
                    let active = store.inhibited();
                    let count = store.inhibitor_count();
                    (removed, active, count)
                }
                Err(err) => {
                    return Err(zbus::fdo::Error::AccessDenied(err.message()));
                }
            }
        };
        if !removed {
            // Unknown IDs are treated as a no-op to keep clients resilient
            return Ok(());
        }
        self.emit_inhibitor_updates(active, count, "removed").await;
        Ok(())
    }

    async fn emit_inhibitor_updates(&self, active: bool, count: u32, action: &'static str) {
        match SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH) {
            Ok(ctx) => {
                // Broadcast inhibitor updates so UI clients can refresh badges
                if let Err(err) = ControlServer::inhibitors_changed(&ctx, active, count).await {
                    warn!(
                        ?err,
                        inhibitor_count = count,
                        action,
                        "inhibitor state changed but inhibitors_changed signal fanout failed"
                    );
                }
            }
            Err(err) => {
                warn!(
                    ?err,
                    action,
                    "inhibitor state changed but failed to build signal context for inhibitors_changed"
                );
            }
        }
        // Mutation is already committed; signal fanout is best-effort
        if let Err(err) = self.state.emit_state_changed().await {
            warn!(
                ?err,
                action,
                "inhibitor state changed but post-commit state_changed signal fanout failed"
            );
        }
    }
}
