//! Top-level refresh flow for a reusable notification row

use gtk::prelude::*;
use tokio::sync::mpsc;
use unixnotis_core::hooks;
use unixnotis_ui::presentation::{
    default_activation::DefaultActionTarget, NotificationPresentation,
};

use crate::control::UiCommand;
use crate::ui::icons::IconResolver;

use super::super::super::super::item::RowData;
use super::super::state::{IconSignature, NotificationRowWidgets};
use super::actions::{update_actions, visible_action_count_from};
use super::labels::update_notification_text;
use super::metadata::update_metadata_labels;
use super::thumbnail::{has_content_thumbnail, panel_lead_visual, PanelLeadVisual};
use super::visual::{apply_visual_state, set_widget_visible_if_changed};

pub(in crate::ui::notifications) fn clear_notification_row(
    row: &NotificationRowWidgets,
    icon_resolver: &IconResolver,
) {
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
    row.thumbnail
        .remove_css_class(hooks::panel_card::CONTENT_IMAGE);
    row.thumbnail
        .remove_css_class(hooks::panel_card::SENDER_VISUAL);

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
    icon_resolver.clear_identity_badge(&row.icon);
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
    // A cleared recycled row must release its previous natural height
    row.text_stack.queue_resize();
    row.card.queue_resize();
    row.card_plate.queue_resize();
    row.stack.queue_resize();
    row.root.queue_resize();
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
        clear_notification_row(row, icon_resolver);
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
    // Identity visibility follows block assembly, not stack depth
    let show_identity = !data.app_header_present;
    let has_actions = visible_action_count_from(&presentation, data.is_active) > 0;
    // The daemon has already assigned the visual role after attribution and safe decoding
    let lead_visual = panel_lead_visual(
        &presentation,
        data.presentation.show_avatar,
        data.presentation.show_thumbnail,
    );
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
    update_metadata_labels(row, data, notification, &presentation);
    row.notify_key.set(notification.key());
    update_actions(
        row,
        command_tx,
        notification_snapshot,
        &presentation,
        data.is_active,
    );

    // Text and action changes must not restart an unchanged icon pipeline
    let next_sig = IconSignature::from_presentation(notification, &presentation);
    let mut sig_guard = row.icon_sig.borrow_mut();
    if show_identity && sig_guard.as_ref() != Some(&next_sig) {
        let scale = row.card.scale_factor();
        icon_resolver.apply_identity_badge(
            &row.icon,
            notification,
            presentation.identity.badge,
            presentation.trust.level,
            20,
            scale,
        );
        *sig_guard = Some(next_sig);
    } else if !show_identity {
        // Grouped rows do not own the application icon anymore
        icon_resolver.clear_identity_badge(&row.icon);
        *sig_guard = None;
    }
    set_widget_visible_if_changed(&row.icon, show_identity);
    set_widget_visible_if_changed(&row.app_label, show_identity);
    // Group rows keep the measured top lane so dismiss never covers message text
    set_widget_visible_if_changed(&row.header, true);
    set_widget_visible_if_changed(&row.close_button, true);
    // Clear paintable state before selecting a new role on a recycled row
    row.thumbnail.clear();
    row.thumbnail
        .remove_css_class(hooks::panel_card::CONTENT_IMAGE);
    row.thumbnail
        .remove_css_class(hooks::panel_card::SENDER_VISUAL);
    let mut actual_visual = lead_visual;
    match actual_visual {
        PanelLeadVisual::ConversationAvatar => {
            icon_resolver.apply_sender_visual(&row.thumbnail, notification);
        }
        PanelLeadVisual::ContentImage => {
            row.thumbnail
                .add_css_class(hooks::panel_card::CONTENT_IMAGE);
            icon_resolver.apply_content_visual(&row.thumbnail, notification);
        }
        PanelLeadVisual::DecorativeSenderVisual => {
            icon_resolver.apply_sender_visual(&row.thumbnail, notification);
            row.thumbnail
                .add_css_class(hooks::panel_card::SENDER_VISUAL);
        }
        PanelLeadVisual::None => {}
    }

    // A malformed preferred avatar must not hide independently valid content media
    if actual_visual == PanelLeadVisual::ConversationAvatar
        && row.thumbnail.paintable().is_none()
        && data.presentation.show_thumbnail
        && has_content_thumbnail(&presentation)
    {
        row.thumbnail.clear();
        row.thumbnail
            .remove_css_class(hooks::panel_card::SENDER_VISUAL);
        row.thumbnail
            .add_css_class(hooks::panel_card::CONTENT_IMAGE);
        icon_resolver.apply_content_visual(&row.thumbnail, notification);
        if row.thumbnail.paintable().is_some() {
            actual_visual = PanelLeadVisual::ContentImage;
        } else {
            // Invalid content stays out of the lane instead of reserving a blank slot
            row.thumbnail
                .remove_css_class(hooks::panel_card::CONTENT_IMAGE);
            actual_visual = PanelLeadVisual::None;
        }
    }

    // A semantic role is not enough to reserve a slot; the bounded texture must exist
    let has_thumbnail =
        actual_visual != PanelLeadVisual::None && row.thumbnail.paintable().is_some();
    apply_visual_state(
        row,
        data,
        notification,
        &presentation,
        has_actions,
        has_thumbnail,
    );
    // Keep malformed or empty rasters out of the visible lead lane
    set_widget_visible_if_changed(&row.thumbnail, has_thumbnail);
    set_widget_visible_if_changed(&row.card_plate, true);
    set_widget_visible_if_changed(&row.card, true);
    // Recycled rows can change natural height when text, media, or stack depth changes
    row.text_stack.queue_resize();
    row.card.queue_resize();
    row.card_plate.queue_resize();
    row.stack.queue_resize();
    row.root.queue_resize();
}
