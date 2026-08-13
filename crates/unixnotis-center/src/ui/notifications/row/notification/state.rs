//! Shared notification-row state and limits
//!
//! This file keeps the reusable row widgets and small helper structs together

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use unixnotis_core::{NotificationKey, NotificationView};
use unixnotis_ui::presentation::default_activation::DefaultActionBinding;
use unixnotis_ui::presentation::{BadgePresentation, NotificationPresentation, TrustLevel};

use super::reply::InlineReplyWidgets;

pub(in crate::ui::notifications) struct NotificationRowWidgets {
    // Active rows use one shared generation-bound card activation binding
    pub(in crate::ui::notifications) default_activation: DefaultActionBinding,
    // The real ListView child owns vertical spacing and recycled-row geometry
    pub(super) root: gtk::Box,
    // Same-cell grid measures every visible stack layer as one row
    pub(super) stack: gtk::Grid,
    // Styled notification card inside the ListView row wrapper
    pub(super) card: gtk::Box,
    // Polygon wrapper clips both visual output and pointer hit testing
    pub(super) card_plate: unixnotis_ui::CutCorner,
    // Collapsed groups use two non-interactive rear silhouettes
    pub(super) stack_middle: gtk::Box,
    pub(super) stack_back: gtk::Box,
    // Main icon shown at the top-left of the row
    pub(super) icon: gtk::Image,
    // Identity header compacts only for collapsed rows owned by a group header
    pub(super) header: gtk::Box,
    // App name text shown beside the icon
    pub(super) app_label: gtk::Label,
    // Application identity is rendered by the shared block header
    pub(super) secondary_claim: gtk::Label,
    pub(super) trust_chip: gtk::Label,
    // Critical badge remains allocated so urgency changes only toggle visibility
    pub(super) urgency_badge: gtk::Label,
    // Dismiss remains in the measured header and targets the exact generation
    pub(super) close_button: gtk::Button,
    // Optional metadata rows are present for themes but hidden unless config enables them
    pub(super) meta_top: gtk::Box,
    // Optional top metadata label for category/urgency styling
    pub(super) meta_label: gtk::Label,
    // Optional relative time badge shown when metadata is enabled
    pub(super) time_badge: gtk::Label,
    // Optional large image preview for notifications with image hints
    pub(super) thumbnail: gtk::Image,
    // Message column keeps text and actions beside a lead visual
    pub(super) text_stack: gtk::Box,
    // Summary line with stronger visual weight
    pub(super) summary_label: gtk::Label,
    // Body text section that can span multiple lines
    pub(super) body_label: gtk::Label,
    // Arrival-time popup explanation remains visible after runtime state changes
    pub(super) popup_status: gtk::Label,
    pub(super) footer: gtk::Box,
    // Optional footer metadata hooks for theme-specific chips
    pub(super) footer_left: gtk::Label,
    pub(super) footer_right: gtk::Label,
    // Container for optional action buttons
    pub(super) actions_box: gtk::Box,
    // Live-only reply form is kept outside the action button cache
    pub(super) inline_reply: InlineReplyWidgets,
    // Exact notification identity bound to this reused row widget
    pub(super) notify_key: Rc<Cell<NotificationKey>>,
    // Recycled rows must rebuild action closures when the notification generation changes
    pub(super) action_cache_key: Cell<NotificationKey>,
    // Last rendered action signature for cheap no-op detection
    pub(super) action_cache:
        RefCell<Vec<(String, String, unixnotis_core::ApplicationActionPolicy)>>,
    // Reply metadata and live state are cached separately from ordinary actions
    pub(super) reply_cache: RefCell<(
        unixnotis_core::InlineReply,
        unixnotis_core::InlineReplyPolicy,
        bool,
    )>,
    // Last rendered icon signature so decode work only happens on a real change
    pub(super) icon_sig: RefCell<Option<IconSignature>>,
}

// Hard caps keep very large payloads from blowing up row height
pub(super) const MAX_SUMMARY_LABEL_CHARS: usize = 160;
pub(super) const MAX_BODY_LABEL_CHARS: usize = 512;
// Action labels stay bounded so one button cannot distort the whole row
pub(super) const MAX_ACTION_LABEL_CHARS: usize = 20;

pub(super) struct OptionalLabelState<'a> {
    // Hidden rows should collapse instead of leaving dead card spacing
    pub(super) visible: bool,
    // Borrow when possible so repeated row refreshes do not allocate
    pub(super) text: Cow<'a, str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ui::notifications) struct IconSignature {
    // Every field that can change the chosen header icon belongs in this key
    badge_icon: String,
    desktop_id: String,
    claimed_theme_icon: String,
    claimed_desktop_id: String,
    presentation: BadgePresentation,
    trust: TrustLevel,
}

impl IconSignature {
    pub(super) fn from_presentation(
        notification: &NotificationView,
        presentation: &NotificationPresentation,
    ) -> Self {
        // Signature includes all fields that can change icon resolution output
        // Reuse the row presentation so icon checks do not rebuild all labels and actions
        Self {
            badge_icon: notification.attribution.badge_icon.clone(),
            desktop_id: notification.attribution.desktop_id.clone(),
            claimed_theme_icon: notification.image.claimed_theme_icon.clone(),
            claimed_desktop_id: notification.image.claimed_desktop_id.clone(),
            presentation: presentation.identity.badge,
            trust: presentation.trust.level,
        }
    }
}
