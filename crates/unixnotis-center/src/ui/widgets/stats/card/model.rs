//! Retained state for one statistic card

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use unixnotis_core::{CommandSpec, StatWidgetConfig, WidgetPluginConfig};

use super::super::builtin::BuiltinStat;
use crate::ui::widgets::utils::RefreshBackoff;

#[derive(Clone)]
pub(in crate::ui::widgets::stats) struct StatItem {
    // Raw config supplies source selection and display metadata
    pub(in crate::ui::widgets::stats) config: StatWidgetConfig,
    // Root card inserted into the grid
    pub(in crate::ui::widgets::stats) root: gtk::Box,
    // Label receives the latest rendered sample
    pub(in crate::ui::widgets::stats) value_label: gtk::Label,
    // Built-in reader state is retained across samples
    pub(in crate::ui::widgets::stats) builtin: Rc<RefCell<Option<BuiltinStat>>>,
    // In-flight state prevents overlapping refreshes
    pub(in crate::ui::widgets::stats) inflight: Rc<Cell<bool>>,
    // Last good value avoids unnecessary relayout
    pub(in crate::ui::widgets::stats) last_value: Rc<RefCell<Option<String>>>,
    // Backoff slows sources whose output remains stable
    pub(in crate::ui::widgets::stats) refresh_backoff: Rc<RefCell<RefreshBackoff>>,
}

pub(super) enum StatSourceRef<'a> {
    Plugin(&'a WidgetPluginConfig),
    Builtin(BuiltinStat),
    Command(&'a CommandSpec),
    Missing,
}
