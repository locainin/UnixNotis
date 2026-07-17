use super::search::send_filter_event;
use crate::control::UiEvent;

#[test]
fn filter_event_sends_exact_query_without_waiting() {
    let (event_tx, event_rx) = async_channel::bounded(1);

    send_filter_event(&event_tx, "terminal".to_string());

    let event = event_rx.try_recv().expect("filter event should be queued");
    assert!(matches!(event, UiEvent::FilterChanged(query) if query == "terminal"));
}

#[test]
fn filter_event_ignores_closed_channel() {
    let (event_tx, event_rx) = async_channel::bounded(1);
    drop(event_rx);

    send_filter_event(&event_tx, "ignored".to_string());
}
