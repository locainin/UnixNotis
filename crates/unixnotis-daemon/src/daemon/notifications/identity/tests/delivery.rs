use super::*;
use crate::daemon::notifications::identity::{SenderMetadata, SenderMetadataStatus};

fn cache_sender(cache: &SenderMetadataCache, connection: &Connection, start_time: u64) {
    let sender = connection
        .unique_name()
        .expect("test connection should have a unique name")
        .to_string();
    cache.insert(
        sender.clone(),
        SenderMetadata {
            sender_name: Some(sender),
            sender_pid: Some(std::process::id()),
            sender_start_time: Some(start_time),
            sender_uid: Some(rustix::process::geteuid().as_raw()),
            status: SenderMetadataStatus::Complete,
            ..SenderMetadata::default()
        },
    );
}

#[test]
fn callback_credentials_require_every_process_lifetime_component() {
    assert!(credentials_match_lifetime(42, 42, Some(7), Some(7), 7));
    assert!(!credentials_match_lifetime(41, 42, Some(7), Some(7), 7));
    assert!(!credentials_match_lifetime(42, 42, Some(6), Some(7), 7));
    assert!(!credentials_match_lifetime(42, 42, Some(7), Some(6), 7));
}

#[tokio::test]
async fn callback_destination_follows_same_process_to_a_new_bus_name() {
    let cache = SenderMetadataCache::new();
    let first = Connection::session()
        .await
        .expect("first session connection");
    let retained = first
        .unique_name()
        .expect("first connection unique name")
        .to_string();
    let start_time = read_process_start_time(std::process::id())
        .expect("current process should expose a start time");
    cache_sender(&cache, &first, start_time);
    first.close().await.expect("close first connection");

    let second = Connection::session()
        .await
        .expect("second session connection");
    cache_sender(&cache, &second, start_time);
    let destination = resolve_callback_destination(
        &cache,
        &second,
        Some(&retained),
        Some(std::process::id()),
        Some(start_time),
    )
    .await
    .expect("same process lifetime should resolve its new address");

    assert_eq!(
        destination.as_str(),
        second
            .unique_name()
            .expect("second connection unique name")
            .as_str()
    );
}

#[tokio::test]
async fn callback_destination_rejects_a_different_process_lifetime() {
    let cache = SenderMetadataCache::new();
    let connection = Connection::session().await.expect("session connection");
    let start_time = read_process_start_time(std::process::id())
        .expect("current process should expose a start time");
    cache_sender(&cache, &connection, start_time);

    let destination = resolve_callback_destination(
        &cache,
        &connection,
        connection.unique_name().map(|name| name.as_str()),
        Some(std::process::id()),
        Some(start_time.saturating_add(1)),
    )
    .await;

    assert!(destination.is_none());
}

#[tokio::test]
async fn callback_destination_keeps_the_exact_live_name_without_lifetime_evidence() {
    let cache = SenderMetadataCache::new();
    let connection = Connection::session().await.expect("session connection");
    let retained = connection
        .unique_name()
        .expect("session connection unique name")
        .to_string();

    let destination =
        resolve_callback_destination(&cache, &connection, Some(&retained), None, None)
            .await
            .expect("an exact live address should remain usable");

    assert_eq!(destination.as_str(), retained);
}

#[tokio::test]
async fn callback_destination_without_lifetime_evidence_never_rebinds() {
    let cache = SenderMetadataCache::new();
    let connection = Connection::session().await.expect("session connection");
    let start_time = read_process_start_time(std::process::id())
        .expect("current process should expose a start time");
    cache_sender(&cache, &connection, start_time);

    let destination =
        resolve_callback_destination(&cache, &connection, Some(":1.999999"), None, None).await;

    assert!(destination.is_none());
}

#[tokio::test]
async fn callback_destination_tries_older_process_candidates_after_a_stale_newest_entry() {
    let cache = SenderMetadataCache::new();
    let connection = Connection::session().await.expect("session connection");
    let start_time = read_process_start_time(std::process::id())
        .expect("current process should expose a start time");
    cache_sender(&cache, &connection, start_time);
    cache.insert(
        ":1.999999".to_string(),
        SenderMetadata {
            sender_name: Some(":1.999999".to_string()),
            sender_pid: Some(std::process::id()),
            sender_start_time: Some(start_time),
            sender_uid: Some(rustix::process::geteuid().as_raw()),
            status: SenderMetadataStatus::Complete,
            ..SenderMetadata::default()
        },
    );

    let destination = resolve_callback_destination(
        &cache,
        &connection,
        Some(":1.retired"),
        Some(std::process::id()),
        Some(start_time),
    )
    .await
    .expect("an older verified address should survive a stale cache entry");

    assert_eq!(
        destination.as_str(),
        connection
            .unique_name()
            .expect("session connection unique name")
            .as_str()
    );
}
