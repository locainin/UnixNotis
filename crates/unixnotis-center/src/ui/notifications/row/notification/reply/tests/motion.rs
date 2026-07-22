//! Inline reply reduced-motion tests

use std::rc::Rc;

use unixnotis_core::Action;

use crate::ui::icons::IconResolver;
use crate::ui::notifications::test_support::init_gtk;

use super::super::state::INLINE_REPLY_TRANSITION_MS;
use super::{
    build_notification_row, row_data, sample_notification, update_notification_row, RowFlags,
};

#[gtk::test]
fn inline_reply_revealer_tracks_runtime_reduced_motion() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = build_notification_row(command_tx.clone());
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    }];
    notification.inline_reply.available = true;
    let notification = Rc::new(notification);

    update_notification_row(
        &row,
        &row_data(
            notification.clone(),
            RowFlags {
                is_active: true,
                reduced_motion: true,
                ..Default::default()
            },
        ),
        &IconResolver::new(),
        &command_tx,
    );
    assert_eq!(row.inline_reply.revealer.transition_duration(), 0);

    update_notification_row(
        &row,
        &row_data(
            notification,
            RowFlags {
                is_active: true,
                ..Default::default()
            },
        ),
        &IconResolver::new(),
        &command_tx,
    );
    assert_eq!(
        row.inline_reply.revealer.transition_duration(),
        INLINE_REPLY_TRANSITION_MS
    );
}
