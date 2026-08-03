use std::cell::RefCell;
use std::collections::HashMap;

use unixnotis_core::{NotificationImage, NotificationView};
use unixnotis_ui::icons::DesktopIconIndex;

use super::{icon_name_is_usable, IconResolverInner};
use crate::ui::icons::cache::IconCache;
use crate::ui::icons::decode::{IconUpdate, IconWorker};
use crate::ui::icons::missing::MissingIconCache;

#[test]
fn empty_icon_name_is_not_resolved() {
    assert!(!icon_name_is_usable(""));
}

#[test]
fn nonempty_icon_name_is_resolved_without_rewriting() {
    assert!(icon_name_is_usable("application-x-executable"));
}

fn resolver_inner(update_tx: async_channel::Sender<IconUpdate>) -> IconResolverInner {
    IconResolverInner {
        desktop_index: DesktopIconIndex::new(),
        cache: RefCell::new(IconCache::new(16)),
        inflight: RefCell::new(HashMap::new()),
        missing_names: RefCell::new(MissingIconCache::new(16)),
        worker: IconWorker::new(update_tx),
    }
}

#[gtk::test]
fn sender_paths_are_not_resolved_by_client_icon_lookup() {
    let (update_tx, _update_rx) = async_channel::bounded(1);
    let resolver = resolver_inner(update_tx);
    let notification = NotificationView {
        id: 1,
        generation: 1,
        app_name: "Icon test".to_string(),
        attribution: unixnotis_core::NotificationAttribution {
            // Keep the daemon-owned fallback empty so this test isolates sender paths
            badge_icon: String::new(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        summary: String::new(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };

    assert!(resolver.resolve_badge(&notification, 16, 1).is_none());
}

#[gtk::test]
fn standard_theme_icon_name_resolves_through_the_resolver() {
    let (update_tx, _update_rx) = async_channel::bounded(1);
    let resolver = resolver_inner(update_tx);
    let resolution = resolver
        .resolve_icon_name("folder", 24, 1)
        .or_else(|| resolver.resolve_icon_name("folder-symbolic", 24, 1));
    assert!(resolution.is_some());
}
