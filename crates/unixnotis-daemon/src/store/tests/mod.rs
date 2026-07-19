//! Store regression coverage and persistence validation

use super::rules::contains_ci;
use super::state::{PersistedDndState, DND_STATE_FILE, DND_STATE_VERSION};
use super::NotificationStore;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use unixnotis_core::{CloseReason, Config, InhibitMode, Notification, NotificationImage, Urgency};
use zbus::zvariant::OwnedValue;

mod dnd;
mod inhibit;
mod lifecycle;
mod ownership;
mod reply;
mod rules;
mod support;

use support::*;
