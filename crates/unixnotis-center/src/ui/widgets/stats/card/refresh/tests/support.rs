//! Shared GTK card fixtures

use crate::ui::widgets::command_runtime::backoff::RefreshBackoff;
use crate::ui::widgets::stats::builtin::BuiltinStat;
use crate::ui::widgets::stats::card::StatItem;
use unixnotis_core::StatWidgetConfig;

static GTK_INIT: std::sync::Once = std::sync::Once::new();

pub(super) fn init_gtk() {
    GTK_INIT.call_once(|| {
        gtk::init().expect("gtk should initialize under the test display");
    });
}

pub(super) fn stat_item(builtin: Option<BuiltinStat>, value: Option<&str>) -> StatItem {
    init_gtk();
    let rendered = value.unwrap_or("n/a");
    StatItem {
        config: StatWidgetConfig::default(),
        root: gtk::Box::new(gtk::Orientation::Vertical, 0),
        value_label: gtk::Label::new(Some(rendered)),
        builtin: std::rc::Rc::new(std::cell::RefCell::new(builtin)),
        inflight: std::rc::Rc::new(std::cell::Cell::new(true)),
        last_value: std::rc::Rc::new(std::cell::RefCell::new(
            value.map(std::string::ToString::to_string),
        )),
        refresh_backoff: std::rc::Rc::new(std::cell::RefCell::new(RefreshBackoff::default())),
    }
}
