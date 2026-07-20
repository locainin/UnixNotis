//! Statistic card refresh dispatch and scheduling gates

mod builtin;
mod command;
mod plugin;

use std::time::{Duration, Instant};

use gtk::prelude::*;
use unixnotis_core::PanelDebugLevel;

use super::{StatItem, StatSourceRef};
use crate::diagnostics::panel_debug as debug;
use crate::ui::widgets::stats::builtin::{BuiltinStat, BuiltinStatKey};
use crate::ui::widgets::utils::INFLIGHT_REFRESH_RECHECK;

impl StatItem {
    pub(in crate::ui::widgets::stats) fn has_builtin_source(&self) -> bool {
        self.config.plugin.is_none() && self.builtin.borrow().is_some()
    }

    pub(in crate::ui::widgets::stats) fn is_grouped_builtin(
        &self,
        now: Instant,
        force: bool,
    ) -> bool {
        if !self.has_builtin_source() || !self.root.is_visible() {
            return false;
        }

        if self.inflight.get() {
            // Groups keep their own in-flight guard
            return true;
        }

        self.refresh_backoff.borrow().should_refresh(now, force)
    }

    pub(in crate::ui::widgets::stats) fn take_builtin_refresh(
        &self,
        now: Instant,
        force: bool,
    ) -> Option<(BuiltinStatKey, BuiltinStat)> {
        if !self.root.is_visible()
            || self.config.plugin.is_some()
            || !self.refresh_backoff.borrow().should_refresh(now, force)
            || self.inflight.get()
        {
            return None;
        }

        let builtin = self.builtin.borrow_mut().take()?;
        self.inflight.set(true);
        Some((builtin.key(), builtin))
    }

    pub(in crate::ui::widgets::stats) fn refresh(&self, base_interval: Duration, force: bool) {
        if !self.root.is_visible() {
            return;
        }
        let now = Instant::now();
        if !self.refresh_backoff.borrow().should_refresh(now, force) {
            return;
        }
        debug::log(PanelDebugLevel::Verbose, || {
            format!("stat refresh: {}", self.config.label)
        });
        if self.inflight.get() {
            return;
        }
        match self.source() {
            StatSourceRef::Plugin(plugin) => self.refresh_plugin(plugin, base_interval),
            StatSourceRef::Builtin(builtin) => self.refresh_builtin(builtin, base_interval),
            StatSourceRef::Command(command) => self.refresh_command(command, base_interval),
            StatSourceRef::Missing => self.refresh_missing(base_interval),
        }
    }

    fn source(&self) -> StatSourceRef<'_> {
        if let Some(plugin) = self.config.plugin.as_ref() {
            // Plugin configuration always has source precedence
            return StatSourceRef::Plugin(plugin);
        }
        if let Some(builtin) = self.builtin.borrow_mut().take() {
            return StatSourceRef::Builtin(builtin);
        }
        self.config
            .cmd
            .as_ref()
            .map_or(StatSourceRef::Missing, StatSourceRef::Command)
    }

    pub(in crate::ui::widgets::stats) fn refresh_missing(&self, base_interval: Duration) {
        // Missing sources settle on the placeholder without spinning
        let changed = self.apply_value("n/a");
        self.refresh_backoff
            .borrow_mut()
            .note_success(Instant::now(), base_interval, changed);
    }

    pub(in crate::ui::widgets::stats) fn next_refresh_in(&self, now: Instant) -> Option<Duration> {
        if !self.root.is_visible() {
            return None;
        }
        if self.inflight.get() {
            // Slow sources should not create a tight scheduler loop
            return Some(INFLIGHT_REFRESH_RECHECK);
        }
        self.refresh_backoff
            .borrow()
            .next_due_in(now)
            .or(Some(Duration::ZERO))
    }
}
