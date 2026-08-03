//! UI event dispatch and list wiring for `UiState`
//!
//! Centralizes `UiEvent` handling so UI state transitions remain coherent and
//! traceable in logs

use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::PanelDebugLevel;

use crate::control::UiEvent;

use super::{panel, UiState};

impl UiState {
    pub fn handle_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Disconnected => {
                debug!("UnixNotis control service disconnected");
                // Old rows and state must not survive into a later daemon generation
                self.list.clear_for_disconnect();
                self.mark_notifications_changed();
                self.update_state(unixnotis_core::ControlState::default());
                self.refresh_counts();
            }
            UiEvent::Seed {
                state,
                active,
                history,
            } => {
                debug!(
                    active = active.len(),
                    history = history.len(),
                    "received initial state"
                );
                // Seed list data before applying state to keep counts aligned
                self.list.seed(active, history);
                self.mark_notifications_changed();
                self.update_state(state);
                self.refresh_counts();
            }
            UiEvent::NotificationAdded(notification) => {
                debug!(
                    id = notification.id,
                    app = %notification.app_name,
                    "notification added"
                );
                self.log_debug(PanelDebugLevel::Verbose, || {
                    format!(
                        "notification added: {} #{}",
                        notification.app_name, notification.id
                    )
                });
                self.list.add_or_update(notification, true);
                self.mark_notifications_changed();
                // Header count reflects the combined active + history totals
                self.refresh_counts();
            }
            UiEvent::NotificationUpdated(notification) => {
                debug!(
                    id = notification.id,
                    app = %notification.app_name,
                    "notification updated"
                );
                self.log_debug(PanelDebugLevel::Verbose, || {
                    format!(
                        "notification updated: {} #{}",
                        notification.app_name, notification.id
                    )
                });
                self.list.add_or_update(notification, true);
                self.mark_notifications_changed();
                // Updates may shift groups; refresh count even when list is stable
                self.refresh_counts();
            }
            UiEvent::NotificationClosed(key, reason) => {
                debug!(
                    id = key.id,
                    generation = key.generation,
                    ?reason,
                    "notification closed"
                );
                self.log_debug(PanelDebugLevel::Verbose, || {
                    format!("notification closed: #{} ({reason:?})", key.id)
                });
                self.list.mark_closed(key, reason);
                self.mark_notifications_changed();
                // Marking closed can move entries between active/history buckets
                self.refresh_counts();
            }
            UiEvent::StateChanged(state) => {
                debug!(
                    dnd = state.dnd_enabled,
                    inhibited = state.inhibited,
                    inhibitors = state.inhibitor_count,
                    "state updated"
                );
                self.log_debug(PanelDebugLevel::Info, || {
                    format!(
                        "state changed: dnd={}, inhibited={}, inhibitors={}",
                        state.dnd_enabled, state.inhibited, state.inhibitor_count
                    )
                });
                self.update_state(state);
                // Keep counts in sync if daemon state changes imply list updates
                self.refresh_counts();
            }
            UiEvent::PanelRequested(request) => {
                debug!(?request, "panel request");
                self.log_debug(PanelDebugLevel::Info, || {
                    format!("panel request: {request:?}")
                });
                // Delegate to visibility handler to keep behavior consistent
                self.apply_panel_request(request);
            }
            UiEvent::GroupToggled(key) => {
                debug!(app = %key, "group toggled");
                self.log_debug(PanelDebugLevel::Verbose, || format!("group toggled: {key}"));
                self.list.toggle_group(&key);
                self.mark_notifications_changed();
                // Toggling can change grouped visibility; counts reflect total entries
                self.refresh_counts();
            }
            UiEvent::MediaUpdated(infos) => {
                debug!(players = infos.len(), "media updated");
                self.log_debug(PanelDebugLevel::Verbose, || {
                    format!("media updated: {} players", infos.len())
                });
                // Avoid updating hidden widgets; cache the snapshot and apply on next open
                // This keeps background CPU minimal while preserving the most recent media state
                if self.panel_visible {
                    if let Some(widget) = self.media.as_mut() {
                        widget.update(&infos);
                    }
                } else {
                    // Cache the latest snapshot to prevent repeated UI work while hidden
                    self.pending_media = Some(infos);
                    self.pending_media_cleared = false;
                }
            }
            UiEvent::MediaCleared => {
                debug!("media cleared");
                self.log_debug(PanelDebugLevel::Info, || "media cleared".to_string());
                // Clearing removes UI state; defer until visible to avoid hidden updates
                // The pending flags ensure the next open matches daemon state
                if self.panel_visible {
                    if let Some(widget) = self.media.as_mut() {
                        widget.clear();
                    }
                } else {
                    // Clear cached data and mark a pending clear so stale artwork is not shown later
                    self.pending_media = None;
                    self.pending_media_cleared = true;
                }
            }
            UiEvent::ClickOutside => {
                debug!("click outside detected");
                // Close requests go through visibility handler to respect guards
                self.close_if_click_outside();
            }
            UiEvent::WorkAreaUpdated(reserved) => {
                debug!(?reserved, "work area updated");
                // Skip reapplying the same margins to avoid redundant layout recalculations
                // Prevents subtle width jitter when Hyprland reports identical values repeatedly
                if self.work_area == reserved {
                    return;
                }
                self.work_area = reserved;
                // Re-apply panel sizing only when the work area actually changes
                // Avoids redundant calls that can cascade into GTK relayout passes
                panel::geometry::apply_panel_config(&self.panel, &self.config, self.work_area);
                let message = format!("work area update: {:?}", self.work_area);
                self.log_debug(PanelDebugLevel::Info, move || message);
            }
            UiEvent::RefreshWidgets => {
                // One-shot timers are re-armed after each refresh tick
                self.refresh_source = None;
                // Track actual timer fire count to spot reschedule churn
                crate::diagnostics::performance::refresh_timer_fired();
                if self.panel_visible {
                    self.refresh_widgets(false);
                    self.start_refresh_timer();
                }
            }
            UiEvent::FilterChanged(query) => {
                if self.list.set_filter_query(&query) {
                    self.mark_notifications_changed();
                    self.log_debug(PanelDebugLevel::Verbose, || {
                        format!("notification filter updated: '{query}'")
                    });
                    // Counts derive from list data and stay accurate before the GTK rebuild lands
                    self.refresh_counts();
                }
            }
            UiEvent::WidgetsCollapsed(collapsed) => {
                self.set_widgets_collapsed(collapsed);
            }
            UiEvent::CssReload => {
                debug!("css reload requested");
                let _report = self.reload_css();
                self.log_debug(PanelDebugLevel::Info, || "css reloaded".to_string());
            }
            UiEvent::ConfigReload => {
                debug!("config reload requested");
                match self.reload_config() {
                    super::reload::ConfigReloadOutcome::Applied { diagnostics, css } => {
                        debug!(
                            diagnostics = diagnostics.len(),
                            css_layers = css.layers.len(),
                            "config reload applied"
                        );
                    }
                    super::reload::ConfigReloadOutcome::Rejected { failure } => {
                        super::reload::log_reload_rejection(&failure);
                    }
                }
            }
        }
    }

    pub fn flush_list_rebuild(&mut self) {
        self.flush_list_rebuild_with_policy(ScrollResetPolicy::NearTopOnly);
    }

    pub(in crate::ui) fn flush_list_rebuild_with_policy(&mut self, policy: ScrollResetPolicy) {
        let snap_to_top = self.panel_visible && should_snap_to_top(&self.panel.sections.scroller);
        let generation = self.notification_rebuild_generation.get().wrapping_add(1);
        self.notification_rebuild_generation.set(generation);
        self.list.flush_rebuild();
        if matches!(policy, ScrollResetPolicy::Force) || snap_to_top {
            reset_notification_scroll(
                &self.panel.sections.scroller,
                self.notification_rebuild_generation.clone(),
                generation,
                policy,
            );
        }
    }

    pub(in crate::ui) const fn mark_notifications_changed(&mut self) {
        if !self.panel_visible {
            self.notifications_changed_while_hidden = true;
        }
    }

    pub const fn list_needs_rebuild(&self) -> bool {
        self.list.needs_rebuild()
    }
}

fn should_snap_to_top(scroller: &gtk::ScrolledWindow) -> bool {
    let adjustment = scroller.vadjustment();
    should_snap_to_top_value(adjustment.value(), adjustment.lower())
}

const fn should_snap_to_top_value(value: f64, lower: f64) -> bool {
    value <= lower + 18.0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui) enum ScrollResetPolicy {
    // Live updates preserve a meaningful position once the user scrolls away
    NearTopOnly,
    // Hidden reseeds invalidate the old position and must show the first row
    Force,
}

pub(in crate::ui) fn reset_notification_scroll(
    scroller: &gtk::ScrolledWindow,
    rebuild_generation: std::rc::Rc<std::cell::Cell<u64>>,
    expected_generation: u64,
    policy: ScrollResetPolicy,
) {
    let scroller = scroller.clone();
    gtk::glib::idle_add_local_once(move || {
        // Layout work can yield to a real user scroll before this callback runs
        // Recheck both the rebuild and scroll state so stale work cannot win
        if should_apply_scroll_reset(
            rebuild_generation.get(),
            expected_generation,
            &scroller,
            policy,
        ) {
            let adjustment = scroller.vadjustment();
            adjustment.set_value(adjustment.lower());
        }
    });
}

fn should_apply_scroll_reset(
    current_generation: u64,
    expected_generation: u64,
    scroller: &gtk::ScrolledWindow,
    policy: ScrollResetPolicy,
) -> bool {
    scroll_reset_generation_is_current(current_generation, expected_generation)
        && (matches!(policy, ScrollResetPolicy::Force) || should_snap_to_top(scroller))
}

const fn scroll_reset_generation_is_current(current: u64, expected: u64) -> bool {
    current == expected
}

#[cfg(test)]
#[path = "events/tests/mod.rs"]
mod tests;
