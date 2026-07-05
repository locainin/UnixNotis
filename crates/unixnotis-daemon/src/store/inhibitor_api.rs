use unixnotis_core::InhibitMode;

use super::store_inhibit::{inhibits_popups, Inhibitor, InhibitorOwnerMismatch};
use super::NotificationStore;

impl NotificationStore {
    pub fn add_inhibitor(&mut self, owner: String, reason: String, scope: u32) -> u64 {
        // Token is monotonic and never reused in-process
        let id = self.next_inhibitor_id.max(1);
        self.next_inhibitor_id = id.saturating_add(1);
        // Save owner so cleanup on disconnect can remove all related inhibitors
        self.inhibitors.insert(
            id,
            Inhibitor {
                id,
                owner,
                reason,
                scope,
            },
        );
        self.refresh_inhibit_state();
        id
    }

    pub fn remove_inhibitor(
        &mut self,
        id: u64,
        owner: &str,
    ) -> Result<bool, InhibitorOwnerMismatch> {
        // Missing token is not an error for idempotent clients
        let Some(existing) = self.inhibitors.get(&id) else {
            return Ok(false);
        };
        // Owner check blocks one client from removing another client inhibitor
        if existing.owner != owner {
            return Err(InhibitorOwnerMismatch::new(
                existing.owner.clone(),
                owner.to_string(),
            ));
        }
        self.inhibitors.remove(&id);
        self.refresh_inhibit_state();
        Ok(true)
    }

    pub fn remove_inhibitors_by_owner(&mut self, owner: &str) -> bool {
        let before = self.inhibitors.len();
        // retain keeps a single in-place pass with no temporary allocations
        self.inhibitors
            .retain(|_, inhibitor| inhibitor.owner != owner);
        if self.inhibitors.len() == before {
            return false;
        }
        // Refresh cached counters only when the set actually changed
        self.refresh_inhibit_state();
        true
    }

    pub fn list_inhibitors(&self) -> Vec<(u64, String, u32, String)> {
        // Snapshot clone prevents external callers from mutating internal state
        let mut inhibitors = Vec::with_capacity(self.inhibitors.len());
        for inhibitor in self.inhibitors.values() {
            inhibitors.push((
                inhibitor.id,
                inhibitor.reason.clone(),
                inhibitor.scope,
                inhibitor.owner.clone(),
            ));
        }
        // Stable order keeps CLI output and tests deterministic
        inhibitors.sort_by_key(|(id, _, _, _)| *id);
        inhibitors
    }

    pub(super) fn should_drop_inhibited(&self) -> bool {
        // DropAll means suppression happens before insertion and history work
        self.inhibited && matches!(self.config.inhibit.mode, InhibitMode::DropAll)
    }

    fn refresh_inhibit_state(&mut self) {
        // Cached values avoid repeated scans during notify hot path checks
        self.inhibitor_count = self.inhibitors.len() as u32;
        self.inhibited = self
            .inhibitors
            .values()
            .any(|inhibitor| inhibits_popups(inhibitor.scope));
    }
}
