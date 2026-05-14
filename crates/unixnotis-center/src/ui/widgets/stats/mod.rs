//! Statistic widgets and refresh orchestration

mod build;
mod card;
mod stats_builtin;
#[cfg(test)]
#[path = "tests/grid.rs"]
mod tests;
mod worker;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use unixnotis_core::StatWidgetConfig;

use self::stats_builtin::{BuiltinStat, BuiltinStatKey};
use super::utils::RefreshBackoff;

pub struct StatGrid {
    // FlowBox root is embedded by the panel widget tree
    root: gtk::FlowBox,
    // Per-stat item state is retained for refresh scheduling
    items: Vec<StatItem>,
}

#[derive(Clone)]
struct StatItem {
    // Raw config is retained for command and plugin selection plus labels
    config: StatWidgetConfig,
    // Root card inserted into the grid
    root: gtk::Box,
    // Render target for the latest stat value
    value_label: gtk::Label,
    // Optional builtin reader reused across refresh calls
    builtin: Rc<RefCell<Option<BuiltinStat>>>,
    // Guard prevents overlapping command or builtin reads
    inflight: Rc<Cell<bool>>,
    // Cached value avoids unnecessary relayout for unchanged results
    last_value: Rc<RefCell<Option<String>>>,
    // Backoff reduces repeated reads when the value is stable
    refresh_backoff: Rc<RefCell<RefreshBackoff>>,
}

struct BuiltinStatJob {
    // Builtin reader variant to execute on the worker thread
    stat: BuiltinStat,
    // One-shot response channel used to return the sampled value
    respond: async_channel::Sender<(BuiltinStat, String)>,
}

struct BuiltinStatWorker {
    // Bounded queue feeding the dedicated builtin worker thread
    tx: crossbeam_channel::Sender<BuiltinStatJob>,
    // True when worker startup failed and callers should read inline
    inline_fallback: bool,
    // Test-only receiver guard keeps the queue alive when no workers are spawned
    #[cfg(test)]
    #[allow(dead_code)]
    receiver_guard: crossbeam_channel::Receiver<BuiltinStatJob>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuiltinSubmitOutcome {
    // Job was accepted by the worker queue
    Submitted,
    // Queue is healthy but currently saturated
    QueueFull,
    // Worker is unavailable and caller must use inline fallback
    WorkerUnavailable,
}

fn apply_cached_value(label: &gtk::Label, cache: &Rc<RefCell<Option<String>>>) {
    if let Some(value) = cache.borrow().as_ref() {
        if label.text().as_str() != value {
            label.set_text(value);
        }
    } else if label.text().as_str() != "n/a" {
        label.set_text("n/a");
    }
}

struct BuiltinRefreshGroup {
    // One live builtin reader is enough for all cards that point at the same source
    stat: BuiltinStat,
    // Every item in the group receives the same sampled value and updated reader state
    items: Vec<StatItem>,
}

fn collect_builtin_groups(
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
