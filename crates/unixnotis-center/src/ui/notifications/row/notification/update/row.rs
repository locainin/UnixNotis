//! Top-level refresh flow for a reusable notification row

use gtk::prelude::*;
use tokio::sync::mpsc;
use unixnotis_ui::presentation::{
    apply_semantic_badge, default_activation::DefaultActionTarget, NotificationPresentation,
};

use crate::control::UiCommand;
use crate::ui::icons::IconResolver;

use super::super::super::super::item::RowData;
use super::super::state::{IconSignature, NotificationRowWidgets};
use super::actions::{update_actions, visible_action_count};
use super::labels::update_notification_text;
use super::metadata::update_metadata_labels;
use super::thumbnail::notification_has_thumbnail;
use super::visual::{apply_visual_state, set_widget_visible_if_changed};

pub(in crate::ui::notifications) fn clear_notification_row(row: &NotificationRowWidgets) {
    // Clear every visible lane before a recycled row can be painted again
    row.default_activation.set_target(None);
    row.notify_key.set(unixnotis_core::NotificationKey {
        id: 0,
        generation: 0,
    });
    row.action_cache_key.set(unixnotis_core::NotificationKey {
        id: 0,
        generation: 0,
    });
    row.action_cache.borrow_mut().clear();
    *row.reply_cache.borrow_mut() = (
        unixnotis_core::InlineReply::default(),
        unixnotis_core::InlineReplyPolicy::Deny,
        false,
    );
    row.icon_sig.borrow_mut().take();
    row.inline_reply.reset_for_recycle();

    for widget in [
        row.card.upcast_ref::<gtk::Widget>(),
        row.card_plate.upcast_ref::<gtk::Widget>(),
        row.header.upcast_ref::<gtk::Widget>(),
        row.meta_top.upcast_ref::<gtk::Widget>(),
        row.footer.upcast_ref::<gtk::Widget>(),
        row.actions_box.upcast_ref::<gtk::Widget>(),
        row.thumbnail.upcast_ref::<gtk::Widget>(),
        row.popup_status.upcast_ref::<gtk::Widget>(),
        row.stack_middle.upcast_ref::<gtk::Widget>(),
        row.stack_back.upcast_ref::<gtk::Widget>(),
    ] {
        widget.set_visible(false);
    }
    row.icon.clear();
    row.thumbnail.clear();
    for label in [
        &row.app_label,
        &row.secondary_claim,
        &row.trust_chip,
        &row.summary_label,
        &row.body_label,
        &row.popup_status,
        &row.meta_label,
        &row.time_badge,
        &row.footer_left,
        &row.footer_right,
    ] {
        label.set_text("");
        label.set_visible(false);
    }
    row.app_label.set_tooltip_text(None);
    while let Some(child) = row.actions_box.first_child() {
        row.actions_box.remove(&child);
    }
}

pub(in crate::ui::notifications) fn update_notification_row(
    row: &NotificationRowWidgets,
    data: &RowData,
    icon_resolver: &IconResolver,
    command_tx: &mpsc::Sender<UiCommand>,
) {
    row.inline_reply
        .set_reduced_motion(data.presentation.reduced_motion);
    // Model changes may briefly update a recycled row without notification data
    let Some(notification_snapshot) = data.notification.as_ref() else {
        clear_notification_row(row);
        return;
    };
    let notification = notification_snapshot.as_ref();
    let presentation = NotificationPresentation::from_view(notification);
    let default_target = data
        .is_active
        .then(|| {
            presentation
                .actions
                .default_key
                .as_ref()
                .map(|action_key| DefaultActionTarget {
                    notification: notification.key(),
                    action_key: action_key.clone(),
                })
        })
        .flatten();
    // Set this before action-cache early returns so recycled rows cannot retain
    // a previous notification generation
    row.default_activation.set_target(default_target);
    let show_identity = !data.collapsed_group_preview && !data.expanded;
    let has_actions = visible_action_count(notification, data.is_active) > 0;
    let has_thumbnail =
        data.presentation.show_thumbnail && notification_has_thumbnail(notification);

    apply_visual_state(row, data, notification, has_actions, has_thumbnail);
    update_notification_text(
        row,
        &presentation.identity.primary_label,
        &presentation.title,
        presentation.body.as_deref().unwrap_or_default(),
        presentation.popup_status.as_deref(),
    );
    if presentation.trust.details_label.is_none() {
        row.app_label.set_tooltip_text(None);
    } else if let Some(details) = presentation.trust.details_label.as_deref() {
        row.app_label.set_tooltip_text(Some(details));
    }
    super::labels::set_label_text_if_changed(
        &row.secondary_claim,
        presentation
            .identity
            .secondary_claim
            .as_deref()
            .unwrap_or_default(),
    );
    super::labels::set_label_visible_if_changed(
        &row.secondary_claim,
        show_identity && presentation.identity.secondary_claim.is_some(),
    );
    super::labels::set_label_text_if_changed(
        &row.trust_chip,
        presentation
            .trust
            .short_label
            .as_deref()
            .unwrap_or_default(),
    );
    super::labels::set_label_visible_if_changed(
        &row.trust_chip,
        show_identity && presentation.trust.short_label.is_some(),
    );
    update_metadata_labels(row, data, notification);
    row.notify_key.set(notification.key());
    update_actions(row, command_tx, notification_snapshot, data.is_active);

    // Text and action changes must not restart an unchanged icon pipeline
    let next_sig = IconSignature::from(notification);
    let mut sig_guard = row.icon_sig.borrow_mut();
    if show_identity && sig_guard.as_ref() != Some(&next_sig) {
        if apply_semantic_badge(&row.icon, presentation.identity.badge, 20) {
            row.icon.set_visible(true);
        } else {
            let scale = row.card.scale_factor();
            // Verified rows keep authenticated application art from the shared resolver
            icon_resolver.apply_badge(&row.icon, notification, 20, scale);
        }
        *sig_guard = Some(next_sig);
    } else if !show_identity {
        *sig_guard = None;
    }
    set_widget_visible_if_changed(&row.icon, show_identity);
    set_widget_visible_if_changed(&row.app_label, show_identity);
    // Group rows keep the measured top lane so dismiss never covers message text
    set_widget_visible_if_changed(&row.header, true);
    set_widget_visible_if_changed(&row.close_button, true);
    if has_thumbnail {
        // Reapply visible thumbnails so config reloads cannot leave stale previews
        let scale = row.card.scale_factor();
        icon_resolver.apply_icon(&row.thumbnail, notification, 56, scale);
    }
    set_widget_visible_if_changed(&row.thumbnail, has_thumbnail);
    set_widget_visible_if_changed(&row.card_plate, true);
    set_widget_visible_if_changed(&row.card, true);
}
