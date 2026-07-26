//! Store regression coverage and persistence validation

use super::state::{PersistedDndState, DND_STATE_FILE, DND_STATE_VERSION};
use super::NotificationStore;
use chrono::Utc;
use std::collections::HashMap;
use unixnotis_core::{Config, InhibitMode, Notification, NotificationImage, Urgency};
use zbus::zvariant::OwnedValue;

mod dnd;
mod inhibit;
pub(in crate::store) mod support;

use support::*;
