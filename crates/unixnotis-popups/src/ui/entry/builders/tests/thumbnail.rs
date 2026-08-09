use super::{append_thumbnail, should_append_thumbnail};
use crate::ui::entry::presentation::{PopupEntryViewModel, PopupKind, ReplyPresentation};
use gtk::prelude::*;
use unixnotis_core::{NotificationImage, NotificationView};
use unixnotis_ui::presentation::{
    BadgePresentation, SenderVisualPresentation, ThumbnailKind, TrustLevel, TrustPresentation,
    VisualPresentation,
};

#[test]
fn application_provided_visual_cannot_enter_message_thumbnail_lane() {
    let mut view = view_model();
    view.visuals.sender = SenderVisualPresentation::ApplicationProvidedIcon;

    assert!(!should_append_thumbnail(&view));
}

#[test]
fn genuine_content_image_enters_message_thumbnail_lane() {
    let mut view = view_model();
    view.thumbnail = ThumbnailKind::Content;
    view.visuals.content_image = true;

    assert!(should_append_thumbnail(&view));
}

#[gtk::test]
fn append_thumbnail_rejects_application_visual_without_adding_widget() {
    let mut notification = notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ApplicationProvidedIcon;
    notification.image.sender_visual = pixel();
    let mut view = view_model();
    view.visuals.sender = SenderVisualPresentation::ApplicationProvidedIcon;
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);

    assert!(!append_thumbnail(&notification, &view, &content));
    assert!(content.first_child().is_none());
}

#[gtk::test]
fn append_thumbnail_adds_only_genuine_content_image() {
    let mut notification = notification();
    notification.image.content_image = pixel();
    let mut view = view_model();
    view.thumbnail = ThumbnailKind::Content;
    view.visuals.content_image = true;
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);

    assert!(append_thumbnail(&notification, &view, &content));
    let image = content
        .first_child()
        .and_downcast::<gtk::Image>()
        .expect("content lane should contain one image");
    assert!(image.has_css_class("unixnotis-popup-content-image"));
}

fn view_model() -> PopupEntryViewModel {
    PopupEntryViewModel {
        kind: PopupKind::Communication,
        app_label: "Example Chat".to_string(),
        secondary_claim: None,
        badge: BadgePresentation::UnknownApplication,
        timestamp_label: "now".to_string(),
        title: "Conversation".to_string(),
        body: Some("Message".to_string()),
        thumbnail: ThumbnailKind::None,
        visuals: VisualPresentation {
            sender: SenderVisualPresentation::None,
            content_image: false,
        },
        default_action_key: None,
        primary_actions: Vec::new(),
        overflow_actions: Vec::new(),
        trust: TrustPresentation {
            level: TrustLevel::Unresolved,
            short_label: Some("Unverified".to_string()),
            details_label: None,
            reply: ReplyPresentation::Hidden,
        },
        critical: false,
    }
}

fn notification() -> NotificationView {
    NotificationView {
        id: 1,
        generation: 1,
        app_name: "Example Chat".to_string(),
        attribution: unixnotis_core::NotificationAttribution::unresolved(
            "Example Chat",
            unixnotis_core::AttributionReason::MissingSenderEvidence,
            "no sender evidence",
            "claim:example-chat".to_string(),
        ),
        summary: "Conversation".to_string(),
        body: "Message".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        category: "im.received".to_string(),
        is_transient: false,
        received_at_unix_seconds: 1_000,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    }
}

fn pixel() -> unixnotis_core::ImageData {
    unixnotis_core::ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![1, 2, 3, 255],
    }
}
