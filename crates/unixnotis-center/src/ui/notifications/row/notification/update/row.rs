//! Top-level refresh flow for a reusable notification row

use gtk::prelude::*;
use tokio::sync::mpsc;

use crate::control::UiCommand;
use crate::ui::icons::IconResolver;

use super::super::super::super::item::RowData;
use super::super::state::{IconSignature, NotificationRowWidgets};
use super::actions::{update_actions, visible_action_count};
use super::labels::update_notification_text;
use super::metadata::update_metadata_labels;
use super::thumbnail::notification_has_thumbnail;
use super::visual::{apply_visual_state, set_widget_visible_if_changed};

pub(in crate::ui::notifications) fn update_notification_row(
    row: &NotificationRowWidgets,
    data: &RowData,
    icon_resolver: &IconResolver,
    command_tx: &mpsc::Sender<UiCommand>,
) {
    // Model changes may briefly update a recycled row without notification data
    let Some(notification_snapshot) = data.notification.as_ref() else {
        return;
    };
    let notification = notification_snapshot.as_ref();
    let has_actions = visible_action_count(notification, data.is_active) > 0;
    let has_thumbnail =
        data.presentation.show_thumbnail && notification_has_thumbnail(notification);

    apply_visual_state(row, data, notification, has_actions, has_thumbnail);
    update_notification_text(
        row,
        &notification.app_name,
        &notification.summary,
        &notification.body,
    );
    update_metadata_labels(row, data, notification);
    row.notify_id.set(notification.id);
    update_actions(row, command_tx, notification_snapshot, data.is_active);

    // Text and action changes must not restart an unchanged icon pipeline
    let next_sig = IconSignature::from(notification);
    let mut sig_guard = row.icon_sig.borrow_mut();
    if sig_guard.as_ref() != Some(&next_sig) {
        let scale = row.card.scale_factor();
        icon_resolver.apply_icon(&row.icon, notification, 22, scale);
        *sig_guard = Some(next_sig);
    }
    if has_thumbnail {
        // Reapply visible thumbnails so config reloads cannot leave stale previews
        let scale = row.card.scale_factor();
        icon_resolver.apply_icon(&row.thumbnail, notification, 56, scale);
    }
    set_widget_visible_if_changed(&row.thumbnail, has_thumbnail);
}
