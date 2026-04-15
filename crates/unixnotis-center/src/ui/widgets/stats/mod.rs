//! Statistic widgets and refresh orchestration

mod build;
mod card;
mod stats_builtin;
#[cfg(test)]
mod tests;
mod worker;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use unixnotis_core::StatWidgetConfig;

use self::stats_builtin::BuiltinStat;
use super::utils::RefreshBackoff;

pub struct StatGrid {
    // FlowBox root is embedded by the panel widget tree
    root: gtk::FlowBox,
    // Per-stat item state is retained for refresh scheduling
    items: Vec<StatItem>,
}

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

fn apply_cached_value(label: &gtk::Label, cache: &Rc<RefCell<Option<String>>>) {
    if let Some(value) = cache.borrow().as_ref() {
        if label.text().as_str() != value {
            label.set_text(value);
        }
    } else if label.text().as_str() != "n/a" {
        label.set_text("n/a");
    }
}
