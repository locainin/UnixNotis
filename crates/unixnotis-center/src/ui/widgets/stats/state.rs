//! Retained widget and worker state for statistic cards

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use unixnotis_core::StatWidgetConfig;

use super::super::utils::RefreshBackoff;
use super::BuiltinStat;

pub struct StatGrid {
    // FlowBox root is embedded by the panel widget tree
    pub(super) root: gtk::FlowBox,
    // Per-stat item state is retained for refresh scheduling
    pub(super) items: Vec<StatItem>,
}

#[derive(Clone)]
pub(super) struct StatItem {
    // Raw config is retained for command and plugin selection plus labels
    pub(super) config: StatWidgetConfig,
    // Root card inserted into the grid
    pub(super) root: gtk::Box,
    // Render target for the latest stat value
    pub(super) value_label: gtk::Label,
    // Optional builtin reader reused across refresh calls
    pub(super) builtin: Rc<RefCell<Option<BuiltinStat>>>,
    // Guard prevents overlapping command or builtin reads
    pub(super) inflight: Rc<Cell<bool>>,
    // Cached value avoids unnecessary relayout for unchanged results
    pub(super) last_value: Rc<RefCell<Option<String>>>,
    // Backoff reduces repeated reads when the value is stable
    pub(super) refresh_backoff: Rc<RefCell<RefreshBackoff>>,
}

pub(super) struct BuiltinStatJob {
    // Builtin reader variant to execute on the worker thread
    pub(super) stat: BuiltinStat,
    // One-shot response channel used to return the sampled value
    pub(super) respond: async_channel::Sender<(BuiltinStat, String)>,
}

pub(super) struct BuiltinStatWorker {
    // Bounded queue feeding the dedicated builtin worker thread
    pub(super) tx: crossbeam_channel::Sender<BuiltinStatJob>,
    // True when worker startup failed and callers should read inline
    pub(super) inline_fallback: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BuiltinSubmitOutcome {
    // Job was accepted by the worker queue
    Submitted,
    // Queue is healthy but currently saturated
    QueueFull,
    // Worker is unavailable and caller must use inline fallback
    WorkerUnavailable,
}

pub(super) fn apply_cached_value(label: &gtk::Label, cache: &Rc<RefCell<Option<String>>>) {
    if let Some(value) = cache.borrow().as_ref() {
        // Stable values avoid an unnecessary GTK property update
        if label.text().as_str() != value {
            label.set_text(value);
        }
    } else if label.text().as_str() != "n/a" {
        // Missing samples share one predictable fallback label
        label.set_text("n/a");
    }
}
