use super::{SenderMetadataCache, MAX_CACHED_SENDERS};
use crate::daemon::notifications::identity::sender::{
    CommandLineEvidence, SenderMetadata, SenderMetadataStatus,
};

fn metadata(sender: &str, pid: u32) -> SenderMetadata {
    SenderMetadata {
        sender_name: Some(sender.to_string()),
        sender_pid: Some(pid),
        sender_start_time: Some(u64::from(pid)),
        sender_uid: None,
        sender_executable: Some(format!("/usr/bin/app-{pid}")),
        sender_executable_identity: None,
        install_provenance:
            crate::daemon::notifications::identity::desktop_index::InstallProvenance::default(),
        command_line: CommandLineEvidence::default(),
        ancestors: Vec::new(),
        status: SenderMetadataStatus::Complete,
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

#[test]
fn sender_candidates_require_both_pid_and_process_start_time() {
    let cache = SenderMetadataCache::new();
    cache.insert(":1.exact".to_string(), metadata(":1.exact", 42));
    let mut wrong_start = metadata(":1.wrong-start", 42);
    wrong_start.sender_start_time = Some(43);
    cache.insert(":1.wrong-start".to_string(), wrong_start);
    let mut wrong_pid = metadata(":1.wrong-pid", 99);
    wrong_pid.sender_start_time = Some(42);
    cache.insert(":1.wrong-pid".to_string(), wrong_pid);

    assert_eq!(
        cache.sender_candidates_for_process(42, 42, None),
        [":1.exact"]
    );
}

#[test]
fn sender_candidates_exclude_the_retained_address_and_are_newest_first() {
    let cache = SenderMetadataCache::new();
    cache.insert(":1.stale".to_string(), metadata(":1.stale", 42));
    cache.insert(":1.current".to_string(), metadata(":1.current", 42));
    cache.insert(":1.newest".to_string(), metadata(":1.newest", 42));

    assert_eq!(
        cache.sender_candidates_for_process(42, 42, Some(":1.stale")),
        [":1.newest", ":1.current"]
    );
    assert_eq!(
        cache.sender_candidates_for_process(42, 42, Some(":1.newest")),
        [":1.current", ":1.stale"]
    );
}
