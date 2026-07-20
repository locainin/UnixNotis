//! Shared filesystem fixtures for built-in reader tests

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::builtin::BuiltinStat;
use super::super::card::StatItem;
use crate::ui::widgets::utils::RefreshBackoff;
use unixnotis_core::StatWidgetConfig;

static GTK_INIT: std::sync::Once = std::sync::Once::new();

pub(super) fn init_gtk() {
    GTK_INIT.call_once(|| {
        gtk::init().expect("gtk should initialize under the test display");
    });
}

pub(super) fn stat_item(builtin: Option<BuiltinStat>, value: Option<&str>) -> StatItem {
    init_gtk();
    let rendered = value.unwrap_or("n/a");
    StatItem {
        config: StatWidgetConfig::default(),
        root: gtk::Box::new(gtk::Orientation::Vertical, 0),
        value_label: gtk::Label::new(Some(rendered)),
        builtin: std::rc::Rc::new(std::cell::RefCell::new(builtin)),
        inflight: std::rc::Rc::new(std::cell::Cell::new(true)),
        last_value: std::rc::Rc::new(std::cell::RefCell::new(
            value.map(std::string::ToString::to_string),
        )),
        refresh_backoff: std::rc::Rc::new(std::cell::RefCell::new(RefreshBackoff::default())),
    }
}

pub(super) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub(super) fn new(prefix: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir creation failed");
        Self { path }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Cleanup is best effort so a failed assertion remains visible
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn write_device(root: &Path, name: &str, entries: &[(&str, &str)]) {
    let device_path = root.join(name);
    fs::create_dir_all(&device_path).expect("device directory creation failed");
    for (file, contents) in entries {
        fs::write(device_path.join(file), contents).expect("device file write failed");
    }
}
