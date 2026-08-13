//! Panel content and daemon-state synchronization

use gtk::prelude::*;

use crate::ui::UiState;

impl UiState {
    pub const fn panel_is_visible(&self) -> bool {
        self.panel_visible
    }

    pub(in crate::ui) const fn has_any_widgets(&self) -> bool {
        self.volume.is_some()
            || self.brightness.is_some()
            || self.toggles.is_some()
            || self.stats.is_some()
            || self.cards.is_some()
            || (self.media.is_some() && self.config.media.enabled)
    }

    pub(in crate::ui) fn set_widgets_collapsed(&mut self, collapsed: bool) {
        self.widgets_collapsed = collapsed;
        if self.panel.header.actions.focus_toggle.is_active() != collapsed {
            // Mirror external collapse requests into the header toggle state
            self.panel.header.actions.focus_toggle.set_active(collapsed);
        }
        if self.panel.sections.widget_revealer.reveals_child() == collapsed {
            self.panel
                .sections
                .widget_revealer
                .set_reveal_child(!collapsed);
        }
        self.list
            .set_empty_layout(!collapsed && self.has_any_widgets());
    }

    pub(in crate::ui) fn update_state(&mut self, state: unixnotis_core::ControlState) {
        // Dropping the old countdown removes its source unless GLib already stopped it
        drop(self.dnd_expiration_source.take());

        // Avoid re-entrant DND toggles while applying daemon state
        self.dnd_guard.set(true);
        self.panel
            .header
            .actions
            .dnd_toggle
            .set_active(state.dnd_enabled);
        self.dnd_guard.set(false);
        let expires_at = state
            .dnd_enabled
            .then_some(state.dnd_expires_at)
            .filter(|expires_at| *expires_at > 0)
            .unwrap_or(0);
        super::header::dnd::update_dnd_status(&self.panel.header.actions.dnd_status, expires_at);
        if expires_at > 0 {
            self.dnd_expiration_source = Some(super::header::dnd::start_dnd_countdown(
                &self.panel.header.actions.dnd_status,
                expires_at,
            ));
        }
    }

    pub(in crate::ui) fn refresh_counts(&mut self) {
        if !self.panel_visible {
            // Skip label updates while hidden to avoid unnecessary UI work
            // Counts are refreshed on the next open to keep the header accurate
            return;
        }
        let counts = self.list.notification_counts();
        if self.last_count == Some(counts) {
            return;
        }
        self.last_count = Some(counts);
        self.panel.header.count.set_text(&format_counts(counts));
    }
}

fn format_counts(counts: crate::ui::notifications::NotificationCounts) -> String {
    if counts.filter_active {
        return format!("{} / {}", counts.matching, counts.total);
    }
    counts.total.to_string()
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
