//! Statistic widget module wiring

mod build;
mod card;
mod css;
mod group;
mod state;
mod stats_builtin;
#[cfg(test)]
#[path = "tests/grid.rs"]
mod tests;
mod worker;

use self::group::{collect_builtin_groups, BuiltinRefreshGroup};
pub use self::state::StatGrid;
use self::state::StatItem;
use self::state::{apply_cached_value, BuiltinStatJob, BuiltinStatWorker, BuiltinSubmitOutcome};
use self::stats_builtin::{BuiltinStat, BuiltinStatKey};
use super::utils::RefreshBackoff;
