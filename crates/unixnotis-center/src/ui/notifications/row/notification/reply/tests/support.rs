//! Shared fixtures for inline reply tests

use std::rc::Rc;

use unixnotis_core::{InlineReply, NotificationView};

use super::sample_notification;

pub(super) fn drain_main_context() {
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
}

pub(super) fn reply_notification(id: u32, reply: InlineReply) -> Rc<NotificationView> {
    let mut notification = sample_notification();
    notification.id = id;
    notification.inline_reply = reply;
    Rc::new(notification)
}
