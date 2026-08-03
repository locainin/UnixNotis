//! Thumbnail source decisions for notification rows

use unixnotis_ui::presentation::{
    NotificationPresentation, SenderVisualPresentation, ThumbnailKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PanelLeadVisual {
    ConversationAvatar,
    ContentImage,
    DecorativeSenderVisual,
    None,
}

pub(super) fn has_content_thumbnail(presentation: &NotificationPresentation) -> bool {
    // Content thumbnails are already classified by the shared presentation layer
    presentation.media.thumbnail == ThumbnailKind::Content
}

pub(super) const fn has_conversation_avatar(presentation: &NotificationPresentation) -> bool {
    // Conversation photos may occupy the large sender-visual slot
    matches!(
        presentation.visuals.sender,
        SenderVisualPresentation::ConversationAvatar
    )
}

pub(super) const fn has_sender_visual(presentation: &NotificationPresentation) -> bool {
    // Other sender visuals stay decorative and never replace the trusted badge
    matches!(
        presentation.visuals.sender,
        SenderVisualPresentation::ApplicationProvidedIcon
    )
}

pub(super) fn panel_lead_visual(
    presentation: &NotificationPresentation,
    show_avatars: bool,
    show_thumbnails: bool,
) -> PanelLeadVisual {
    // Conversation identity always wins the single master-style lead slot
    if show_avatars && has_conversation_avatar(presentation) {
        return PanelLeadVisual::ConversationAvatar;
    }
    // Content images are optional and come after a conversation avatar
    if show_thumbnails && has_content_thumbnail(presentation) {
        return PanelLeadVisual::ContentImage;
    }
    // Decorative sender art is lower priority than message content
    if show_thumbnails && has_sender_visual(presentation) {
        return PanelLeadVisual::DecorativeSenderVisual;
    }
    PanelLeadVisual::None
}
