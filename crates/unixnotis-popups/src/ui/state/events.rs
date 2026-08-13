//! Top-level popup event routing and fail-soft configuration reloads

use tracing::debug;
use unixnotis_core::{Config, ControlState, PopupGateState};
use unixnotis_ui::css;

use crate::dbus::UiEvent;

use super::super::window::apply_popup_config;
use super::model::UiState;

impl UiState {
    pub fn handle_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Disconnected => {
                debug!("UnixNotis control service disconnected");
                self.control_state = ControlState::default();
                self.hidden_popups.clear();
                self.reconcile_seed(Vec::new());
            }
            UiEvent::Seed { state, active } => {
                // Seed is daemon truth, so filtering uses the newest gate state
                self.control_state = state;
                self.reconcile_seed(active);
            }
            UiEvent::NotificationAdded(notification, show_popup) => {
                if show_popup {
                    debug!(
                        id = notification.id,
                        app = %notification.app_name,
                        "popup added"
                    );
                    self.add_popup(notification);
                }
            }
            UiEvent::NotificationUpdated(notification, show_popup) => {
                debug!(
                    id = notification.id,
                    app = %notification.app_name,
                    "popup updated"
                );
                self.update_popup(notification, show_popup);
            }
            UiEvent::NotificationClosed(key, _reason) => {
                debug!(id = key.id, generation = key.generation, "popup closed");
                self.remove_popup_if_generation(key);
            }
            UiEvent::PopupHidden(key) => {
                debug!(
                    id = key.id,
                    generation = key.generation,
                    "popup banner hidden"
                );
                self.hide_popup_if_generation(key);
            }
            UiEvent::PopupGateChanged(gate) => {
                // Gate updates change only policy fields and preserve unrelated daemon state
                apply_popup_gate(&mut self.control_state, gate);
                self.retain_allowed_popups();
            }
            UiEvent::CssReload => {
                debug!("popup css reload requested");
                let report = self.css.reload(css::DEFAULT_CSS);
                super::super::css_reload::log_reload_failures(&report, "watcher reload");
                self.invalidate_icon_sources();
            }
            UiEvent::ConfigReload => {
                debug!("popup config reload requested");
                self.reload_config();
            }
        }
    }

    fn reload_config(&mut self) {
        // Rejected input must leave the complete previous runtime state active
        let config = match Config::load_from_path(&self.config_path) {
            Ok(config) => config,
            Err(err) => {
                super::super::config_reload::log_config_rejection(&err);
                return;
            }
        };
        let theme_base = match Config::config_dir_for_path(&self.config_path) {
            Ok(path) => path,
            Err(_error) => {
                super::super::config_reload::log_theme_resolution_failure("config-directory");
                return;
            }
        };
        let theme_paths = match config.resolve_theme_paths_from(&theme_base) {
            Ok(paths) => paths,
            Err(_error) => {
                super::super::config_reload::log_theme_resolution_failure("theme-paths");
                return;
            }
        };

        // Config, generated CSS, and geometry move forward as one accepted snapshot
        self.config = config.clone();
        debug!("popup config reloaded");
        self.css.update_theme(theme_paths, config.theme.clone());
        let report = self.css.reload(css::DEFAULT_CSS);
        super::super::css_reload::log_reload_failures(&report, "config reload");
        self.invalidate_icon_sources();
        apply_popup_config(
            &self.popup_window,
            &self.popup_stack,
            &config,
            &self.popup_input_region,
        );
        self.refresh_after_config_reload();
    }
}

pub(super) const fn apply_popup_gate(control_state: &mut ControlState, gate: PopupGateState) {
    // The daemon owns these two fields and sends them as one coherent snapshot
    control_state.dnd_enabled = gate.dnd_enabled;
    control_state.inhibited = gate.inhibited;
}
