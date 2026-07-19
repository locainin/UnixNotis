use super::InlineReply;

#[test]
fn inline_reply_default_is_unavailable_and_carries_no_display_text() {
    let reply = InlineReply::default();

    assert!(!reply.available);
    assert!(reply.label.is_empty());
    assert!(reply.placeholder.is_empty());
    assert!(reply.submit_label.is_empty());
    assert!(reply.submit_icon.is_empty());
}
