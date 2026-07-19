//! Shared refresh grouping for cards backed by the same built-in reader

use std::collections::HashMap;
use std::time::Instant;

use super::{BuiltinStat, BuiltinStatKey, StatItem};

pub(super) struct BuiltinRefreshGroup {
    // One live builtin reader is enough for all cards that point at the same source
    pub(super) stat: BuiltinStat,
    // Every item in the group receives the same sampled value and updated reader state
    pub(super) items: Vec<StatItem>,
}

pub(super) fn collect_builtin_groups(
    items: &[StatItem],
    now: Instant,
    force: bool,
) -> HashMap<BuiltinStatKey, BuiltinRefreshGroup> {
    let mut groups: HashMap<BuiltinStatKey, BuiltinRefreshGroup> = HashMap::new();

    for item in items {
        let Some((key, stat)) = item.take_builtin_refresh(now, force) else {
            continue;
        };

        // Keep one reader per unique builtin source, then fan the result out to every card
        match groups.get_mut(&key) {
            Some(group) => group.items.push(item.clone()),
            None => {
                groups.insert(
                    key,
                    BuiltinRefreshGroup {
                        stat,
                        items: vec![item.clone()],
                    },
                );
            }
        }
    }

    groups
}
