use super::{SenderMetadataCache, MAX_CACHED_SENDERS};
use crate::daemon::notifications::identity::sender::SenderMetadata;

fn metadata(sender: &str, pid: u32) -> SenderMetadata {
    SenderMetadata {
        sender_name: Some(sender.to_string()),
        sender_pid: Some(pid),
        sender_start_time: Some(u64::from(pid)),
        sender_executable: Some(format!("/usr/bin/app-{pid}")),
        sender_executable_identity: None,
    }
}

#[test]
fn sender_cache_reuses_exact_unique_name_and_removes_disconnected_owner() {
    let cache = SenderMetadataCache::new();
    cache.insert(":1.42".to_string(), metadata(":1.42", 42));

    assert_eq!(
        cache.get(":1.42").and_then(|value| value.sender_pid),
        Some(42)
    );
    assert!(cache.get(":1.43").is_none());

    cache.remove(":1.42");
    assert!(cache.get(":1.42").is_none());
}

#[test]
fn sender_cache_evicts_least_recently_used_entry_at_capacity() {
    let cache = SenderMetadataCache::new();
    for index in 0..MAX_CACHED_SENDERS {
        let sender = format!(":1.{index}");
        cache.insert(sender.clone(), metadata(&sender, index as u32 + 1));
    }
    assert!(cache.get(":1.0").is_some());

    cache.insert(
        ":1.replacement".to_string(),
        metadata(":1.replacement", 999),
    );

    assert!(cache.get(":1.1").is_none());
    assert!(cache.get(":1.0").is_some());
    assert!(cache.get(":1.replacement").is_some());
}
