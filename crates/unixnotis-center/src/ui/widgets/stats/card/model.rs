//! Retained state for one statistic card

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use unixnotis_core::StatWidgetConfig;

use super::super::builtin::BuiltinStat;
use crate::ui::widgets::utils::RefreshBackoff;

#[derive(Clone)]
pub(in crate::ui::widgets::stats) struct StatItem {
    // Raw config supplies source selection and display metadata
    pub(super) config: StatWidgetConfig,
    // Root card inserted into the grid
    pub(super) root: gtk::Box,
    // Label receives the latest rendered sample
    pub(super) value_label: gtk::Label,
    // Built-in reader state is retained across samples
    pub(super) builtin: Rc<RefCell<Option<BuiltinStat>>>,
    // In-flight state prevents overlapping refreshes
    pub(super) inflight: Rc<Cell<bool>>,
    // Last good value avoids unnecessary relayout
    pub(super) last_value: Rc<RefCell<Option<String>>>,
    // Backoff slows sources whose output remains stable
    pub(super) refresh_backoff: Rc<RefCell<RefreshBackoff>>,
}
