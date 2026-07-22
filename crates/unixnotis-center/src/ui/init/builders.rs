//! Startup widget builders

use std::rc::Rc;

use gtk::prelude::*;

use super::super::{icons, media, notifications, panel, widgets, UiStateInit};

pub(super) fn build_notification_list(
    panel: &panel::widgets::PanelWidgets,
    init: &UiStateInit,
    icon_resolver: Rc<icons::IconResolver>,
) -> notifications::NotificationList {
    let list_config = notifications::NotificationListConfig {
        max_active: init.config.history.max_active,
        max_entries: init.config.history.max_entries,
        transient_to_history: init.config.history.transient_to_history,
        show_notification_metadata: init.config.panel.notification_metadata_visible,
        notification_metadata: init.config.panel.notification_metadata.clone(),
        notification_corners: init.config.theme.notification_corners,
        show_notification_thumbnails: init.config.panel.notification_thumbnails_visible,
        reduced_motion: init.config.panel.reduced_motion,
        empty_text: init.config.panel.empty_text.clone(),
        no_matching_text: init.config.panel.no_matching_text.clone(),
        empty_offset_top: init.config.panel.empty_offset_top,
        empty_alignment: init.config.panel.empty_alignment,
    };

    // Notification list owns row virtualization and icon resolution
    // Startup only passes the resolved policy and shared channels
    notifications::NotificationList::new(
        panel.sections.scroller.clone(),
        init.command_tx.clone(),
        init.event_tx.clone(),
        icon_resolver,
        list_config,
    )
}

pub(super) fn build_media_widget(
    panel: &panel::widgets::PanelWidgets,
    init: &UiStateInit,
) -> Option<media::MediaWidget> {
    let panel_width = panel::geometry::requested_panel_width(&panel.root);
    let media = init.media_handle.as_ref().map(|handle| {
        media::MediaWidget::new(
            &panel.sections.media_container,
            handle.clone(),
            panel_width,
            &init.config.media,
        )
    });

    if media.is_none() {
        // Hidden container keeps layout stable without reserving blank media space
        panel.sections.media_container.set_visible(false);
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
    panel: &panel::widgets::PanelWidgets,
    init: &UiStateInit,
    icon_resolver: &unixnotis_core::IconAssetResolver,
) -> ExtraWidgets {
    let (volume, brightness) =
        super::super::widget_builders::build_quick_controls(panel, &init.config);
    let (toggles, stats, cards) =
        super::super::widget_builders::build_extra_widgets(panel, &init.config, icon_resolver);

    ExtraWidgets {
        volume,
        brightness,
        toggles,
        stats,
        cards,
    }
}

pub(super) fn icon_resolver_for_widgets(
    config_path: &std::path::Path,
) -> unixnotis_core::IconAssetResolver {
    match unixnotis_core::Config::config_dir_for_path(config_path) {
        Ok(config_dir) => unixnotis_core::IconAssetResolver::new(config_dir),
        Err(err) => {
            // An unknown root disables file assets instead of consulting the process directory
            tracing::warn!(
                ?err,
                "widget icon assets disabled because config dir is unavailable"
            );
            unixnotis_core::IconAssetResolver::disabled()
        }
    }
}

pub(super) fn has_visible_widget_section(panel: &panel::widgets::PanelWidgets) -> bool {
    // Empty-state spacing depends on whether any upper panel section is visible
    panel.sections.quick_controls.get_visible()
        || panel.sections.media_container.get_visible()
        || panel.sections.toggle_container.get_visible()
        || panel.sections.stat_container.get_visible()
        || panel.sections.card_container.get_visible()
}
