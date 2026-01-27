//! Statistic widgets and refresh orchestration.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{glib, Align};
use tracing::warn;
use unixnotis_core::{PanelDebugLevel, StatWidgetConfig};

use crossbeam_channel as channel;

use super::stats_builtin::BuiltinStat;
use super::util::{run_command_capture_async, RefreshBackoff};
use crate::debug;

pub struct StatGrid {
    root: gtk::FlowBox,
    items: Vec<StatItem>,
}

struct StatItem {
    config: StatWidgetConfig,
    root: gtk::Box,
    value_label: gtk::Label,
    builtin: Rc<RefCell<Option<BuiltinStat>>>,
    inflight: Rc<Cell<bool>>,
    last_value: Rc<RefCell<Option<String>>>,
    // Backoff reduces repeated reads when the value is stable.
    refresh_backoff: Rc<RefCell<RefreshBackoff>>,
}

struct BuiltinStatJob {
    stat: BuiltinStat,
    respond: async_channel::Sender<(BuiltinStat, String)>,
}

struct BuiltinStatWorker {
    tx: channel::Sender<BuiltinStatJob>,
    inline_fallback: bool,
    // Test-only receiver guard keeps the queue alive when no workers are spawned.
    #[cfg(test)]
    #[allow(dead_code)]
    receiver_guard: channel::Receiver<BuiltinStatJob>,
}

impl BuiltinStatWorker {
    // Limit queued jobs to avoid unbounded growth if refresh is faster than the worker.
    const QUEUE_CAPACITY: usize = 32;

    // Single worker avoids per-refresh thread churn while keeping UI updates async.
    fn global() -> &'static Self {
        static WORKER: OnceLock<BuiltinStatWorker> = OnceLock::new();
        WORKER.get_or_init(Self::new)
    }

    fn new() -> Self {
        Self::new_with_capacity(Self::QUEUE_CAPACITY, true)
    }

    fn new_with_capacity(capacity: usize, spawn_workers: bool) -> Self {
        // Bounded queue prevents unbounded memory growth during slow reads or tight refresh loops.
        let (tx, rx) = channel::bounded::<BuiltinStatJob>(capacity);
        #[cfg(test)]
        let receiver_guard = rx.clone();
        let inline_fallback = if spawn_workers {
            let spawn = thread::Builder::new()
                .name("unixnotis-builtin-stats".to_string())
                .spawn(move || {
                    for mut job in rx.iter() {
                        let value = job.stat.read().unwrap_or_else(|| "n/a".to_string());
                        let _ = job.respond.send_blocking((job.stat, value));
                    }
                });
            spawn.is_err()
        } else {
            true
        };
        if inline_fallback {
            warn!("builtin stats worker unavailable; using inline reads");
        }

        Self {
            tx,
            inline_fallback,
            #[cfg(test)]
            receiver_guard,
        }
    }

    fn submit(&self, job: BuiltinStatJob) -> bool {
        if self.inline_fallback {
            return false;
        }
        // Avoid blocking the UI thread when the worker queue is saturated.
        self.tx.try_send(job).is_ok()
    }
}

#[cfg(test)]
impl BuiltinStatWorker {
    fn new_for_tests(capacity: usize) -> Self {
        let (tx, rx) = channel::bounded::<BuiltinStatJob>(capacity);
        // Do not spawn a worker; tests drive queue saturation deterministically.
        Self {
            tx,
            inline_fallback: false,
            receiver_guard: rx,
        }
    }
}

impl StatGrid {
    pub fn new(configs: &[StatWidgetConfig]) -> Option<Self> {
        let mut items = Vec::new();
        for config in configs {
            if !config.enabled {
                continue;
            }
            items.push(StatItem::new(config.clone()));
        }
        if items.is_empty() {
            return None;
        }

        let root = gtk::FlowBox::new();
        root.add_css_class("unixnotis-stat-grid");
        root.set_selection_mode(gtk::SelectionMode::None);
        root.set_max_children_per_line(2);
        root.set_min_children_per_line(2);
        root.set_row_spacing(8);
        root.set_column_spacing(8);
        root.set_halign(Align::Fill);
        root.set_hexpand(true);

        for item in &items {
            root.insert(&item.root, -1);
        }

        Some(Self { root, items })
    }

    pub fn root(&self) -> &gtk::FlowBox {
        &self.root
    }

    pub fn refresh(&self, base_interval: Duration, force: bool) {
        for item in &self.items {
            item.refresh(base_interval, force);
        }
    }
}

impl StatItem {
    fn new(config: StatWidgetConfig) -> Self {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.add_css_class("unixnotis-stat-card");
        if config.min_height > 0 {
            card.set_size_request(-1, config.min_height);
        }

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class("unixnotis-stat-header");
        if let Some(icon_name) = config.icon.as_ref() {
            let icon = gtk::Image::from_icon_name(icon_name);
            icon.set_pixel_size(16);
            icon.add_css_class("unixnotis-stat-icon");
            header.append(&icon);
        }

        let title = gtk::Label::new(Some(&config.label));
        title.add_css_class("unixnotis-stat-title");
        title.set_xalign(0.0);
        header.append(&title);

        let value_label = gtk::Label::new(Some("n/a"));
        value_label.add_css_class("unixnotis-stat-value");
        value_label.set_xalign(0.0);
        value_label.set_width_chars(12);

        card.append(&header);
        card.append(&value_label);

        let builtin = config
            .cmd
            .as_ref()
            .and_then(|cmd| BuiltinStat::from_command(cmd));

        Self {
            config,
            root: card,
            value_label,
            builtin: Rc::new(RefCell::new(builtin)),
            inflight: Rc::new(Cell::new(false)),
            last_value: Rc::new(RefCell::new(None)),
            refresh_backoff: Rc::new(RefCell::new(RefreshBackoff::default())),
        }
    }

    fn refresh(&self, base_interval: Duration, force: bool) {
        if !self.root.is_visible() {
            return;
        }
        let now = Instant::now();
        // Skip refresh when the backoff window has not elapsed.
        if !self.refresh_backoff.borrow().should_refresh(now, force) {
            return;
        }
        debug::log(PanelDebugLevel::Verbose, || {
            format!("stat refresh: {}", self.config.label)
        });
        if self.inflight.get() {
            return;
        }
        if let Some(builtin) = self.builtin.borrow_mut().take() {
            self.inflight.set(true);
            let (tx, rx) = async_channel::bounded(1);
            let mut fallback = builtin.clone();
            let worker = BuiltinStatWorker::global();
            if !worker.submit(BuiltinStatJob {
                stat: builtin,
                respond: tx,
            }) {
                self.inflight.set(false);
                // Fallback to inline reads when the worker thread is unavailable.
                let value = fallback.read().unwrap_or_else(|| "n/a".to_string());
                *self.builtin.borrow_mut() = Some(fallback);
                let changed = self.apply_value(&value);
                self.refresh_backoff.borrow_mut().note_success(
                    Instant::now(),
                    base_interval,
                    changed,
                );
                return;
            }

            let label = self.value_label.clone();
            let inflight = self.inflight.clone();
            let builtin_cell = self.builtin.clone();
            let last_value = self.last_value.clone();
            let refresh_backoff = self.refresh_backoff.clone();
            glib::MainContext::default().spawn_local(async move {
                let result = rx.recv().await;
                inflight.set(false);
                let Ok((builtin, value)) = result else {
                    *builtin_cell.borrow_mut() = Some(fallback);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                };
                *builtin_cell.borrow_mut() = Some(builtin);
                if value.is_empty() {
                    apply_cached_value(&label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_success(Instant::now(), base_interval, false);
                } else if last_value.borrow().as_deref() != Some(&value) {
                    label.set_text(&value);
                    *last_value.borrow_mut() = Some(value);
                    refresh_backoff
                        .borrow_mut()
                        .note_success(Instant::now(), base_interval, true);
                } else {
                    refresh_backoff
                        .borrow_mut()
                        .note_success(Instant::now(), base_interval, false);
                }
            });
            return;
        }

        let Some(cmd) = self.config.cmd.as_ref() else {
            let changed = self.apply_value("n/a");
            self.refresh_backoff
                .borrow_mut()
                .note_success(Instant::now(), base_interval, changed);
            return;
        };
        self.inflight.set(true);
        let cmd = cmd.clone();
        let rx = run_command_capture_async(&cmd);
        let label = self.value_label.clone();
        let inflight = self.inflight.clone();
        let last_value = self.last_value.clone();
        let refresh_backoff = self.refresh_backoff.clone();
        glib::MainContext::default().spawn_local(async move {
            let output = match rx.recv().await {
                Ok(output) => output,
                Err(_) => {
                    inflight.set(false);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            inflight.set(false);
            let output = match output {
                Ok(output) => output,
                Err(err) => {
                    warn!(?cmd, ?err, "stat command failed");
                    apply_cached_value(&label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            if !output.status.success() {
                warn!(?cmd, "stat command failed");
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let value = stdout.trim();
            if value.is_empty() {
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, false);
            } else {
                let changed = last_value.borrow().as_deref() != Some(value);
                if changed {
                    label.set_text(value);
                    *last_value.borrow_mut() = Some(value.to_string());
                }
                refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, changed);
            }
        });
    }

    fn apply_value(&self, value: &str) -> bool {
        if self.last_value.borrow().as_deref() == Some(value) {
            return false;
        }
        self.value_label.set_text(value);
        *self.last_value.borrow_mut() = Some(value.to_string());
        true
    }
}

fn apply_cached_value(label: &gtk::Label, cache: &Rc<RefCell<Option<String>>>) {
    if let Some(value) = cache.borrow().as_ref() {
        if label.text().as_str() != value {
            label.set_text(value);
        }
    } else if label.text().as_str() != "n/a" {
        label.set_text("n/a");
    }
}

#[cfg(test)]
mod tests {
    use super::{BuiltinStatJob, BuiltinStatWorker};
    use crate::ui::widgets::stats_builtin::BuiltinStat;

    #[test]
    fn builtin_worker_queue_full_falls_back() {
        let worker = BuiltinStatWorker::new_for_tests(1);
        let stat_a = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
        let stat_b = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
        let (tx_a, _rx_a) = async_channel::bounded(1);
        let (tx_b, _rx_b) = async_channel::bounded(1);

        assert!(worker.submit(BuiltinStatJob {
            stat: stat_a,
            respond: tx_a,
        }));
        assert!(!worker.submit(BuiltinStatJob {
            stat: stat_b,
            respond: tx_b,
        }));
    }
}
