//! UI event dispatch and list wiring for `UiState`.
//!
//! Centralizes `UiEvent` handling so UI state transitions remain coherent and
//! traceable in logs.

use tracing::debug;
use unixnotis_core::PanelDebugLevel;
use unixnotis_ui::css;

use crate::dbus::UiEvent;

use super::{panel, UiState};

impl UiState {
    pub fn handle_event(&mut self, event: UiEvent) {
        match event {
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
                // Seed list data before applying state to keep counts aligned.
                self.list.seed(active, history);
                self.update_state(state);
                self.refresh_counts();
            }
            UiEvent::NotificationAdded(notification, _show_popup) => {
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
                // Header count reflects the combined active + history totals.
                self.refresh_counts();
            }
            UiEvent::NotificationUpdated(notification, _show_popup) => {
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
                // Updates may shift groups; refresh count even when list is stable.
                self.refresh_counts();
            }
            UiEvent::NotificationClosed(id, reason) => {
                debug!(id, ?reason, "notification closed");
                self.log_debug(PanelDebugLevel::Verbose, || {
                    format!("notification closed: #{id} ({reason:?})")
                });
                self.list.mark_closed(id, reason);
                // Marking closed can move entries between active/history buckets.
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
                // Keep counts in sync if daemon state changes imply list updates.
                self.refresh_counts();
            }
            UiEvent::PanelRequested(request) => {
                debug!(?request, "panel request");
                self.log_debug(PanelDebugLevel::Info, || {
                    format!("panel request: {:?}", request)
                });
                // Delegate to visibility handler to keep behavior consistent.
                self.apply_panel_request(request);
            }
            UiEvent::GroupToggled(key) => {
                debug!(app = %key, "group toggled");
                self.log_debug(PanelDebugLevel::Verbose, || format!("group toggled: {key}"));
                self.list.toggle_group(&key);
                // Toggling can change stacked visibility; counts reflect total entries.
                self.refresh_counts();
            }
            UiEvent::MediaUpdated(infos) => {
                debug!(players = infos.len(), "media updated");
                self.log_debug(PanelDebugLevel::Verbose, || {
                    format!("media updated: {} players", infos.len())
                });
                // Avoid updating hidden widgets; cache the snapshot and apply on next open.
                // This keeps background CPU minimal while preserving the most recent media state.
                if self.panel_visible {
                    if let Some(widget) = self.media.as_mut() {
                        widget.update(&infos);
                    }
                } else {
                    // Cache the latest snapshot to prevent repeated UI work while hidden.
                    self.pending_media = Some(infos);
                    self.pending_media_cleared = false;
                }
            }
            UiEvent::MediaCleared => {
                debug!("media cleared");
                self.log_debug(PanelDebugLevel::Info, || "media cleared".to_string());
                // Clearing removes UI state; defer until visible to avoid hidden updates.
                // The pending flags ensure the next open matches daemon state.
                if self.panel_visible {
                    if let Some(widget) = self.media.as_mut() {
                        widget.clear();
                    }
                } else {
                    // Clear cached data and mark a pending clear so stale artwork is not shown later.
                    self.pending_media = None;
                    self.pending_media_cleared = true;
                }
            }
            UiEvent::ClickOutside => {
                debug!("click outside detected");
                // Close requests go through visibility handler to respect guards.
                self.close_if_click_outside();
            }
            UiEvent::WorkAreaUpdated(reserved) => {
                debug!(?reserved, "work area updated");
                self.work_area = reserved;
                panel::apply_panel_config(&self.panel, &self.config, self.work_area);
                let message = format!("work area update: {:?}", self.work_area);
                self.log_debug(PanelDebugLevel::Info, move || message);
            }
            UiEvent::RefreshWidgets => {
                if self.panel_visible {
                    // Timer ticks should be ignored while the panel is hidden.
                    self.refresh_widgets(false);
                }
            }
            UiEvent::CssReload => {
                debug!("css reload requested");
                self.css.reload(css::DEFAULT_CSS);
                self.log_debug(PanelDebugLevel::Info, || "css reloaded".to_string());
            }
            UiEvent::ConfigReload => {
                debug!("config reload requested");
                self.reload_config();
            }
        }
    }

    pub fn flush_list_rebuild(&mut self) {
        self.list.flush_rebuild();
    }

    pub fn list_needs_rebuild(&self) -> bool {
        self.list.needs_rebuild()
    }
}
