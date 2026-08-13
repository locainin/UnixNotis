//! Widget handles owned by the panel header

use super::actions::PanelActionWidgets;
use super::search::PanelSearchWidgets;

pub(in crate::ui) struct PanelHeaderWidgets {
    // Structural handles remain grouped so callers do not rebuild child relationships
    pub(in crate::ui) root: gtk::Box,
    pub(in crate::ui) top: gtk::Box,
    pub(in crate::ui) action_row: gtk::Box,
    pub(in crate::ui) title: gtk::Label,
    pub(in crate::ui) subtitle: gtk::Label,
    pub(in crate::ui) count: gtk::Label,
    // Feature groups own their internal controls and signal state
    pub(in crate::ui) search: PanelSearchWidgets,
    pub(in crate::ui) actions: PanelActionWidgets,
}
