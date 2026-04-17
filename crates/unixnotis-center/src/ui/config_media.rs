//! Media-specific config reload helpers for `UiState`
//!
//! Keeps the main reload module focused on orchestration while this file owns
//! the media shell rebuild rules

use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::Config;

use super::{media_widget, UiState};

impl UiState {
    pub(super) fn apply_media_config(&mut self, config: &Config) {
        if !config.media.enabled {
            self.disable_media_widget();
            return;
        }

        self.panel.media_container.set_visible(true);
        // Rebuilds should follow the real live panel width when GTK already knows it
        let panel_width = super::panel::live_panel_width(&self.panel.root);
        if self.media_layout_changed(config) {
            self.rebuild_media_widget(config, panel_width);
            return;
        }

        self.sync_media_widget(config, panel_width);
    }

    fn disable_media_widget(&mut self) {
        self.panel.media_container.set_visible(false);
        self.clear_media_container();
        self.media = None;
        debug!("media disabled");
    }

    fn media_layout_changed(&self, config: &Config) -> bool {
        self.media
            .as_ref()
            .is_some_and(|media| !media.matches_layout(&config.media))
    }

    fn rebuild_media_widget(&mut self, config: &Config, panel_width: i32) {
        // Layout changes replace the GTK subtree, so preserve the current selection first
        let snapshot = self
            .media
            .as_ref()
            .map(media_widget::MediaWidget::snapshot)
            .unwrap_or_default();

        // The old shell is removed before the new one is attached to avoid duplicate cards
        self.clear_media_container();
        self.media = None;

        let Some(handle) = self.media_handle.as_ref() else {
            // Media runtime comes from startup wiring, so enabling it later needs a restart
            tracing::warn!("media runtime not available; restart required to enable media");
            return;
        };

        debug!("media widget rebuilt for layout change");
        let mut media = media_widget::MediaWidget::new(
            &self.panel.media_container,
            handle.clone(),
            panel_width,
            &config.media,
        );
        if !snapshot.is_empty() {
            // The visible player is restored so reload does not blank the current card
            media.restore_snapshot(&snapshot);
        }
        self.media = Some(media);
    }

    fn sync_media_widget(&mut self, config: &Config, panel_width: i32) {
        match (self.media.as_mut(), self.media_handle.as_ref()) {
            (Some(media), _) => {
                // Reuse the existing shell when only width or metadata flags changed
                debug!("media layout updated");
                media.apply_layout(panel_width, &config.media);
            }
            (None, Some(handle)) => {
                debug!("media widget created");
                let media = media_widget::MediaWidget::new(
                    &self.panel.media_container,
                    handle.clone(),
                    panel_width,
                    &config.media,
                );
                self.media = Some(media);
            }
            (None, None) => {
                // Missing runtime means the feature cannot be created from config reload alone
                tracing::warn!("media runtime not available; restart required to enable media");
            }
        }
    }

    fn clear_media_container(&self) {
        // Rebuilds remove old children one by one so GTK releases the shell cleanly
        while let Some(child) = self.panel.media_container.first_child() {
            self.panel.media_container.remove(&child);
        }
    }
}
