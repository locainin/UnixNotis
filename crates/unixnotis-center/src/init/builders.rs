//! Startup widget builders

use std::rc::Rc;

use gtk::prelude::*;

use super::super::{icons, list, media_widget, panel, widgets, UiStateInit};

pub(super) fn build_notification_list(
    panel: &panel::PanelWidgets,
    init: &UiStateInit,
    icon_resolver: Rc<icons::IconResolver>,
) -> list::NotificationList {
    let list_config = list::NotificationListConfig {
        max_active: init.config.history.max_active,
        max_entries: init.config.history.max_entries,
        transient_to_history: init.config.history.transient_to_history,
        show_notification_metadata: init.config.panel.notification_metadata_visible,
        show_notification_thumbnails: init.config.panel.notification_thumbnails_visible,
        empty_text: init.config.panel.empty_text.clone(),
        empty_offset_top: init.config.panel.empty_offset_top,
    };

    // Notification list owns row virtualization and icon resolution
    // Startup only passes the resolved policy and shared channels
    list::NotificationList::new(
        panel.scroller.clone(),
        init.command_tx.clone(),
        init.event_tx.clone(),
        icon_resolver,
        list_config,
    )
}

pub(super) fn build_media_widget(
    panel: &panel::PanelWidgets,
    init: &UiStateInit,
) -> Option<media_widget::MediaWidget> {
    let panel_width = panel::live_panel_width(&panel.root);
    let media = init.media_handle.as_ref().map(|handle| {
        media_widget::MediaWidget::new(
            &panel.media_container,
            handle.clone(),
            panel_width,
            &init.config.media,
        )
    });

    if media.is_none() {
        // Hidden container keeps layout stable without reserving blank media space
        panel.media_container.set_visible(false);
    }
    media
}

pub(super) struct ExtraWidgets {
    // Volume and brightness use the same command slider implementation
    pub(super) volume: Option<widgets::volume::VolumeWidget>,
    pub(super) brightness: Option<widgets::brightness::BrightnessWidget>,
    // Extra sections are optional and can be disabled independently
    pub(super) toggles: Option<widgets::toggles::ToggleGrid>,
    pub(super) stats: Option<widgets::stats::StatGrid>,
    pub(super) cards: Option<widgets::cards::CardGrid>,
}

pub(super) fn build_widget_sections(
    panel: &panel::PanelWidgets,
    init: &UiStateInit,
) -> ExtraWidgets {
    let (volume, brightness) =
        super::super::widget_builders::build_quick_controls(panel, &init.config);
    let (toggles, stats, cards) =
        super::super::widget_builders::build_extra_widgets(panel, &init.config);

    ExtraWidgets {
        volume,
        brightness,
        toggles,
        stats,
        cards,
    }
}

pub(super) fn has_visible_widget_section(panel: &panel::PanelWidgets) -> bool {
    // Empty-state spacing depends on whether any upper panel section is visible
    panel.quick_controls.get_visible()
        || panel.media_container.get_visible()
        || panel.toggle_container.get_visible()
        || panel.stat_container.get_visible()
        || panel.card_container.get_visible()
}
