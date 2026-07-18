//! Versioned doctor report model, rendering, and command orchestration

mod model;
mod render;
mod runner;
mod text;

pub(super) use model::{DoctorCheck, DoctorLogResult, DoctorLogSource, DoctorSeverity};
pub use runner::run;
pub(super) use text::{redact_home, redact_home_text, safe_doctor_text, truncate_with_ellipsis};

#[cfg(test)]
mod tests;
