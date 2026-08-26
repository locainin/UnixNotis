//! Output formatting helpers for noticenterctl

mod diagnostics;
mod error;
mod gate;
mod notifications;
mod writer;

pub use diagnostics::print_notification_diagnostics;
pub use error::format_cli_error;
pub use gate::require_diagnostic_mode;
pub use notifications::{print_inhibitors, print_notifications};
pub use writer::{write_stderr, write_stdout};
