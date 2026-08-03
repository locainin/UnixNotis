//! Thumbnail source decisions for notification rows

use unixnotis_ui::presentation::{
    NotificationPresentation, SenderVisualPresentation, ThumbnailKind,
};

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
