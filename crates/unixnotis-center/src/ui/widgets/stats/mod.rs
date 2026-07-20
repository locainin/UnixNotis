//! Statistic widget module wiring

mod build;
mod builtin;
mod card;
mod group;
mod state;
mod style;
#[cfg(test)]
#[path = "tests/grid.rs"]
mod tests;
mod worker;

use self::builtin::{BuiltinStat, BuiltinStatKey};
use self::group::{collect_builtin_groups, BuiltinRefreshGroup};
pub use self::state::StatGrid;
use self::state::StatItem;
use self::state::{apply_cached_value, BuiltinStatJob, BuiltinStatWorker, BuiltinSubmitOutcome};
use super::utils::RefreshBackoff;
