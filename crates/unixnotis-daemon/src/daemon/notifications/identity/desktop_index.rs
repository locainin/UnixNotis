//! Desktop application index preserving system and user entry origins

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use gio::prelude::AppInfoExt;

use super::executable::{executable_evidence_for_path, FileIdentity};

const MAX_DESKTOP_FILES: usize = 8_192;

#[derive(Debug, Clone)]
pub(super) struct DesktopRecord {
    pub(super) id: String,
    pub(super) display_name: String,
    pub(super) badge_icon: String,
    pub(super) executable_path: Option<PathBuf>,
    pub(super) executable_identity: Option<FileIdentity>,
    pub(super) system_entry: bool,
    pub(super) dbus_activatable: bool,
    names: HashSet<String>,
}

impl DesktopRecord {
    pub(super) fn claim_matches(&self, claim: &str) -> bool {
        // Normalized aliases cover desktop names without trusting free-form display text
        self.names.contains(&normalize_name(claim))
    }

    #[cfg(test)]
    pub(super) fn fixture(
        id: &str,
        display_name: &str,
        executable_path: &str,
        identity: FileIdentity,
        system_entry: bool,
        dbus_activatable: bool,
    ) -> Self {
        let mut names = HashSet::new();
        names.insert(normalize_name(display_name));
        Self {
            id: id.to_string(),
            display_name: display_name.to_string(),
            badge_icon: id.to_string(),
            executable_path: Some(PathBuf::from(executable_path)),
            executable_identity: Some(identity),
            system_entry,
            dbus_activatable,
            names,
        }
    }
}

#[derive(Debug, Default)]
pub(in crate::daemon) struct DesktopIdentityIndex {
    records: Vec<DesktopRecord>,
    by_id: HashMap<String, Vec<usize>>,
    by_identity: HashMap<(u64, u64), Vec<usize>>,
    system_names: HashSet<String>,
    trusted_relays: Vec<ExecutableIdentity>,
}

#[derive(Debug, Clone)]
struct ExecutableIdentity {
    path: PathBuf,
    identity: FileIdentity,
}

impl DesktopIdentityIndex {
    pub(in crate::daemon) fn shared() -> Arc<Self> {
        static INDEX: OnceLock<Arc<DesktopIdentityIndex>> = OnceLock::new();
        // One immutable snapshot serves the daemon lifetime and every notification burst
        INDEX.get_or_init(|| Arc::new(Self::new())).clone()
    }

    #[must_use]
    pub(in crate::daemon) fn new() -> Self {
        let mut index = Self::default();
        // User entries are scanned first so local desktop overrides keep normal precedence
        for (root, system_entry) in desktop_roots() {
            index.scan_root(&root, system_entry);
            if index.records.len() >= MAX_DESKTOP_FILES {
                break;
            }
        }
        // Relay trust is tied to the installed file identity instead of its basename
        index.index_trusted_relay(Path::new("/usr/bin/notify-send"));
        index.index_trusted_relay(Path::new("/usr/local/bin/notify-send"));
        index
    }

    pub(super) fn records_for_id(&self, id: &str) -> Vec<&DesktopRecord> {
        self.by_id
            .get(&normalize_desktop_id(id))
            .into_iter()
            .flatten()
            .filter_map(|index| self.records.get(*index))
            .collect()
    }

    pub(super) fn records_for_executable(&self, identity: FileIdentity) -> Vec<&DesktopRecord> {
        self.by_identity
            .get(&(identity.device, identity.inode))
            .into_iter()
            .flatten()
            .filter_map(|index| self.records.get(*index))
            .collect()
    }

    pub(super) fn claim_matches_system_app(&self, claim: &str) -> bool {
        self.system_names.contains(&normalize_name(claim))
    }

    pub(super) fn trusted_relay_path(&self, identity: FileIdentity) -> Option<&Path> {
        self.trusted_relays
            .iter()
            .find(|relay| relay.identity.same_file(identity))
            .map(|relay| relay.path.as_path())
    }

    fn scan_root(&mut self, root: &Path, system_entry: bool) {
        // A bounded iterative walk avoids recursion and unlimited desktop-file growth
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                if self.records.len() >= MAX_DESKTOP_FILES {
                    return;
                }
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    pending.push(entry.path());
                    continue;
                }
                if file_type.is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("desktop")
                {
                    self.add_desktop_file(&entry.path(), system_entry);
                }
            }
        }
    }

    fn add_desktop_file(&mut self, path: &Path, system_origin: bool) {
        // GIO applies desktop-entry parsing rules before any identity is indexed
        let Some(desktop) = gio::DesktopAppInfo::from_filename(path) else {
            return;
        };
        let Some(id) = desktop
            .id()
            .map(|value| normalize_desktop_id(value.as_str()))
        else {
            return;
        };
        if id.is_empty() {
            return;
        }
        let display_name = desktop.display_name().to_string();
        let executable_path = desktop_executable(&desktop)
            .as_deref()
            .and_then(resolve_program);
        let executable_identity = executable_path
            .as_deref()
            .and_then(executable_evidence_for_path)
            .map(|evidence| evidence.identity);
        let desktop_identity = executable_evidence_for_path(path).map(|evidence| evidence.identity);
        // System association requires protected metadata and a protected executable
        let system_entry = system_origin
            && desktop_identity.is_some_and(FileIdentity::is_system_managed)
            && executable_identity.is_some_and(FileIdentity::is_system_managed);
        let badge_icon = desktop
            .string("Icon")
            .map_or_else(|| id.clone(), |value| value.to_string());
        let mut names = HashSet::new();
        // Each alias is only a claim matcher after executable identity already agrees
        names.insert(normalize_name(&display_name));
        names.insert(normalize_name(desktop.name().as_str()));
        if let Some(generic_name) = desktop.generic_name() {
            names.insert(normalize_name(generic_name.as_str()));
        }
        if let Some(wm_class) = desktop.startup_wm_class() {
            names.insert(normalize_name(wm_class.as_str()));
        }
        names.insert(normalize_name(&id));
        if let Some(executable) = executable_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
        {
            names.insert(normalize_name(executable));
        }
        names.retain(|name| !name.is_empty());

        let record = DesktopRecord {
            id: id.clone(),
            display_name,
            badge_icon,
            executable_path,
            executable_identity,
            system_entry,
            dbus_activatable: desktop.boolean("DBusActivatable"),
            names,
        };
        let record_index = self.records.len();
        // Protected names help detect spoofing but never establish identity on their own
        if system_entry {
            self.system_names.extend(record.names.iter().cloned());
        }
        self.by_id.entry(id).or_default().push(record_index);
        if let Some(identity) = record.executable_identity {
            self.by_identity
                .entry((identity.device, identity.inode))
                .or_default()
                .push(record_index);
        }
        self.records.push(record);
    }

    fn index_trusted_relay(&mut self, path: &Path) {
        let Some(evidence) = executable_evidence_for_path(path) else {
            return;
        };
        // Writable relay binaries stay ordinary unknown senders
        if evidence.identity.is_system_managed() {
            self.trusted_relays.push(ExecutableIdentity {
                path: evidence.canonical_path,
                identity: evidence.identity,
            });
        }
    }

    #[cfg(test)]
    pub(super) fn from_records(
        records: Vec<DesktopRecord>,
        trusted_relays: Vec<(PathBuf, FileIdentity)>,
    ) -> Self {
        let mut index = Self::default();
        for record in records {
            let record_index = index.records.len();
            if record.system_entry {
                index.system_names.extend(record.names.iter().cloned());
            }
            index
                .by_id
                .entry(normalize_desktop_id(&record.id))
                .or_default()
                .push(record_index);
            if let Some(identity) = record.executable_identity {
                index
                    .by_identity
                    .entry((identity.device, identity.inode))
                    .or_default()
                    .push(record_index);
            }
            index.records.push(record);
        }
        index.trusted_relays = trusted_relays
            .into_iter()
            .map(|(path, identity)| ExecutableIdentity { path, identity })
            .collect();
        index
    }
}

fn desktop_roots() -> Vec<(PathBuf, bool)> {
    let mut roots = Vec::new();
    // The user data root remains distinct because its entries are not system evidence
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
    {
        roots.push((data_home.join("applications"), false));
    }
    let data_dirs =
        std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    roots.extend(std::env::split_paths(&data_dirs).map(|root| (root.join("applications"), true)));
    roots
}

fn resolve_program(program: &Path) -> Option<PathBuf> {
    // Canonical paths are presentation data while device and inode carry the proof
    if program.is_absolute() {
        return program.canonicalize().ok();
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find_map(|candidate| candidate.canonicalize().ok())
}

fn desktop_executable(desktop: &gio::DesktopAppInfo) -> Option<PathBuf> {
    // GIO exposes a nullable executable for valid D-Bus-activated entries without Exec
    desktop.commandline()?;
    let executable = desktop.executable();
    (!executable.as_os_str().is_empty()).then_some(executable)
}

pub(super) fn normalize_desktop_id(value: &str) -> String {
    // Desktop hints commonly include an optional suffix and mixed case
    value
        .trim()
        .strip_suffix(".desktop")
        .unwrap_or_else(|| value.trim())
        .to_ascii_lowercase()
}

pub(super) fn normalize_name(value: &str) -> String {
    // Punctuation and case do not create separate branding aliases
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
#[path = "tests/desktop_index.rs"]
mod tests;
