//! Desktop application metadata shared by panel and popup icon resolution

use std::collections::{HashMap, HashSet};

use gtk::gio;
use gtk::gio::prelude::AppInfoExt;
use gtk::glib::prelude::Cast;

#[derive(Default)]
pub struct DesktopIconIndex {
    names: HashMap<String, Vec<String>>,
    wm_classes: HashMap<String, Vec<String>>,
    ids: HashMap<String, Vec<String>>,
    executables: HashMap<String, Vec<String>>,
}

impl DesktopIconIndex {
    #[must_use]
    pub fn new() -> Self {
        let mut index = Self::default();
        index.rebuild();
        index
    }

    pub fn rebuild(&mut self) {
        // Application installation changes are rare, so rebuilding keeps lookup state coherent
        self.names.clear();
        self.wm_classes.clear();
        self.ids.clear();
        self.executables.clear();
        for app_info in gio::AppInfo::all() {
            let Ok(desktop) = app_info.downcast::<gio::DesktopAppInfo>() else {
                continue;
            };
            let icon_name = desktop
                .string("Icon")
                .map(|value| value.to_string())
                .unwrap_or_default();
            if icon_name.is_empty() {
                continue;
            }
            self.add_name(desktop.name().as_str(), &icon_name);
            self.add_name(desktop.display_name().as_str(), &icon_name);
            if let Some(generic) = desktop.generic_name() {
                self.add_name(generic.as_str(), &icon_name);
            }
            if let Some(startup_wm_class) = desktop.startup_wm_class() {
                self.add_wm_class(startup_wm_class.as_str(), &icon_name);
            }
            if let Some(id) = desktop.id() {
                self.add_id(id.as_str(), &icon_name);
            }
            if let Some(executable) = desktop
                .string("Exec")
                .and_then(|exec| executable_basename(exec.as_str()))
            {
                self.add_executable(&executable, &icon_name);
            }
        }
    }

    #[must_use]
    pub fn icons_for(&self, key: &str) -> Option<Vec<String>> {
        let normalized = normalize_key(key);
        if normalized.is_empty() {
            return None;
        }
        let mut out = Vec::new();
        if let Some(values) = self.ids.get(&normalized) {
            out.extend(values.iter().cloned());
        }
        if let Some(values) = self.wm_classes.get(&normalized) {
            out.extend(values.iter().cloned());
        }
        if let Some(values) = self.names.get(&normalized) {
            out.extend(values.iter().cloned());
        }
        if let Some(values) = self.executables.get(&normalized) {
            out.extend(values.iter().cloned());
        }
        if out.is_empty() {
            return None;
        }
        let mut seen = HashSet::new();
        Some(
            out.into_iter()
                .filter(|value| seen.insert(value.clone()))
                .collect(),
        )
    }

    fn add_name(&mut self, key: &str, icon: &str) {
        add_icon_to_map(&mut self.names, key, icon);
    }

    fn add_wm_class(&mut self, key: &str, icon: &str) {
        add_icon_to_map(&mut self.wm_classes, key, icon);
    }

    fn add_id(&mut self, key: &str, icon: &str) {
        add_icon_to_map(&mut self.ids, key, icon);
        if let Some(stripped) = key.strip_suffix(".desktop") {
            add_icon_to_map(&mut self.ids, stripped, icon);
        }
    }

    fn add_executable(&mut self, key: &str, icon: &str) {
        // Executable basenames come from authenticated daemon metadata
        add_icon_to_map(&mut self.executables, key, icon);
    }
}

fn executable_basename(exec: &str) -> Option<String> {
    // Desktop Exec fields use shell-like quoting plus field-code arguments
    let arguments = shell_words::split(exec).ok()?;
    let program = arguments.first()?;
    std::path::Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn add_icon_to_map(map: &mut HashMap<String, Vec<String>>, key: &str, icon: &str) {
    let key = normalize_key(key);
    if key.is_empty() || icon.is_empty() {
        return;
    }
    let entry = map.entry(key).or_default();
    if !entry.iter().any(|value| value == icon) {
        entry.push(icon.to_string());
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
#[path = "tests/desktop_index.rs"]
mod tests;
