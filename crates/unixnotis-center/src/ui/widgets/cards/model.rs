//! Card grid state shared by construction and refresh logic

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use unixnotis_core::CardWidgetConfig;

use super::super::utils::RefreshBackoff;

pub struct CardGrid {
    // FlowBox root is embedded directly by the panel widget layout
    pub(super) root: gtk::FlowBox,
    // Item list is retained for refresh cadence and due-time aggregation
    pub(super) items: Vec<CardItem>,
}

pub(super) struct CardItem {
    // Raw config is retained for command and plugin refresh decisions
    pub(super) config: CardWidgetConfig,
    // Root card container inserted into the grid
    pub(super) root: gtk::Box,
    // Title line shown in the card header
    pub(super) title_label: gtk::Label,
    // Body label used by non-calendar cards
    pub(super) body_label: gtk::Label,
    // Optional calendar widget for calendar-type cards
    pub(super) calendar: Option<gtk::Calendar>,
    // Fast branch for calendar-specific refresh behavior
    pub(super) is_calendar: bool,
    // Guard blocks overlapping async refresh calls
    pub(super) inflight: Rc<Cell<bool>>,
    // Cached payload avoids visual churn when output is unchanged
    pub(super) last_value: Rc<RefCell<Option<String>>>,
    // Backoff reduces repeated command executions when the value is stable
    pub(super) refresh_backoff: Rc<RefCell<RefreshBackoff>>,
    // Calendar only changes daily; track the last rendered day to avoid redundant updates
    pub(super) last_calendar_day: Rc<Cell<Option<(i32, i32, i32)>>>,
    // Schedules the next calendar update directly at the next local midnight
    pub(super) calendar_next_due: Rc<Cell<Option<Instant>>>,
}
