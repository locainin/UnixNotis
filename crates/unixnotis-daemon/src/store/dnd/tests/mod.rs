//! Do-not-disturb behavior and persistence coverage

use unixnotis_core::Config;

use super::persistence::{PersistedDndState, DND_STATE_FILE, DND_STATE_VERSION};
use crate::store::test_support::*;
use crate::store::NotificationStore;

mod behavior;
mod persistence;
