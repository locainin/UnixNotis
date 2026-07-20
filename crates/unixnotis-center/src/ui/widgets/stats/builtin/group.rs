//! Refresh grouping for cards backed by the same built-in reader

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gtk::glib;

use super::worker::{BuiltinJob, BuiltinWorker, SubmitOutcome};
use super::{BuiltinStat, BuiltinStatKey};
use crate::ui::widgets::stats::card::StatItem;

pub(in crate::ui::widgets::stats) struct RefreshGroup {
    // One reader is enough for every card that points at the same source
    stat: BuiltinStat,
    // Each card receives the same sample and updated reader state
    items: Vec<StatItem>,
}

pub(in crate::ui::widgets::stats) fn collect_builtin_groups(
    items: &[StatItem],
    now: Instant,
    force: bool,
) -> HashMap<BuiltinStatKey, RefreshGroup> {
    let mut groups: HashMap<BuiltinStatKey, RefreshGroup> = HashMap::new();

    for item in items {
        let Some((key, stat)) = item.take_builtin_refresh(now, force) else {
            continue;
        };

        // Keep one reader per source and collect every matching card
        match groups.get_mut(&key) {
            Some(group) => group.items.push(item.clone()),
            None => {
                groups.insert(
                    key,
                    RefreshGroup {
                        stat,
                        items: vec![item.clone()],
                    },
                );
            }
        }
    }

    groups
}

impl RefreshGroup {
    pub(in crate::ui::widgets::stats) fn refresh(self, base_interval: Duration) {
        let (tx, rx) = async_channel::bounded(1);
        let mut fallback = self.stat.clone();
        let worker = BuiltinWorker::global();

        match worker.submit(BuiltinJob {
            stat: self.stat,
            respond: tx,
        }) {
            SubmitOutcome::Submitted => {}
            SubmitOutcome::QueueFull => {
                // Restore every card so the next refresh wave can retry
                for item in self.items {
                    item.restore_builtin_error(fallback.clone(), base_interval);
                }
                return;
            }
            SubmitOutcome::WorkerUnavailable => {
                // Inline fallback samples once before fan-out
                let value = fallback.read().unwrap_or_else(|| "n/a".to_string());
                for item in self.items {
                    item.restore_builtin_value(fallback.clone(), &value, base_interval);
                }
                return;
            }
        }

        glib::MainContext::default().spawn_local(async move {
            let result = rx.recv().await;
            let Ok((builtin, value)) = result else {
                for item in self.items {
                    item.restore_builtin_error(fallback.clone(), base_interval);
                }
                return;
            };

            // Every grouped card receives the same value and reader state
            for item in self.items {
                item.restore_builtin_value(builtin.clone(), &value, base_interval);
            }
        });
    }
}
