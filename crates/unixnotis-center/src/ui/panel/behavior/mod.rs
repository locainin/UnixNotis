//! Panel interaction behavior grouped away from widget construction

mod autoclose;
pub(in crate::ui) mod input;
pub(in crate::ui) mod keyboard;
mod visibility;

pub(in crate::ui) use autoclose::connect_auto_close;
pub(in crate::ui) use keyboard::connect_keyboard_shortcuts;
