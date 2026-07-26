//! Notification lifecycle, ownership, reply, and rule coverage

use std::sync::Arc;

use unixnotis_core::{CloseReason, Config};

use super::rules::contains_ci;
use crate::store::test_support::*;
use crate::store::NotificationStore;

mod lifecycle;
mod ownership;
mod reply;
mod rules;
