//! Command-line control surface for the UnixNotis D-Bus interface.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use gtk::prelude::*;
use gtk::CssProvider;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcCommand;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use unixnotis_core::util;
use unixnotis_core::{
    Config, ControlProxy, NotificationView, PanelDebugLevel, INHIBIT_SCOPE_ALL,
    INHIBIT_SCOPE_POPUPS,
};
use zbus::Connection;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    TogglePanel,
    OpenPanel {
        #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "info")]
        debug: Option<DebugLevelArg>,
    },
    ClosePanel,
    Dnd {
        #[arg(value_enum)]
        state: DndState,
    },
    Clear,
    Dismiss {
        id: u32,
    },
    ListActive {
        #[arg(long)]
        full: bool,
    },
    ListHistory {
        #[arg(long)]
        full: bool,
    },
    Inhibit {
        reason: String,
        #[arg(long, value_enum, default_value = "all")]
        scope: InhibitScopeArg,
    },
    Uninhibit {
        id: u64,
    },
    ListInhibitors,
    CssCheck,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum DndState {
    On,
    Off,
    Toggle,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum DebugLevelArg {
    Critical,
    Warn,
    Info,
    Verbose,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum InhibitScopeArg {
    All,
    Popups,
}

impl InhibitScopeArg {
    fn as_scope(self) -> u32 {
        match self {
            Self::All => INHIBIT_SCOPE_ALL,
            Self::Popups => INHIBIT_SCOPE_POPUPS,
        }
    }
}

impl From<DebugLevelArg> for PanelDebugLevel {
    fn from(value: DebugLevelArg) -> Self {
        match value {
            DebugLevelArg::Critical => PanelDebugLevel::Critical,
            DebugLevelArg::Warn => PanelDebugLevel::Warn,
            DebugLevelArg::Info => PanelDebugLevel::Info,
            DebugLevelArg::Verbose => PanelDebugLevel::Verbose,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if matches!(args.command, Command::CssCheck) {
        run_css_check().context("css-check failed")?;
        return Ok(());
    }
    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    let proxy = ControlProxy::new(&connection)
        .await
        .context("connect to unixnotis control interface")?;

    match args.command {
        Command::TogglePanel => proxy.toggle_panel().await?,
        Command::OpenPanel { debug } => {
            if let Some(level) = debug {
                proxy.open_panel_debug(level.into()).await?;
                follow_debug_logs().context("follow unixnotis debug logs")?;
            } else {
                proxy.open_panel().await?;
            }
        }
        Command::ClosePanel => proxy.close_panel().await?,
        Command::Clear => proxy.clear_all().await?,
        Command::Dismiss { id } => proxy.dismiss(id).await?,
        Command::ListActive { full } => {
            let allow_full = full && util::diagnostic_mode();
            if full && !util::diagnostic_mode() {
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = proxy.list_active().await?;
            print_notifications("active", &notifications, allow_full);
        }
        Command::ListHistory { full } => {
            let allow_full = full && util::diagnostic_mode();
            if full && !util::diagnostic_mode() {
                eprintln!("--full requires UNIXNOTIS_DIAGNOSTIC=1; using redacted output");
            }
            let notifications = proxy.list_history().await?;
            print_notifications("history", &notifications, allow_full);
        }
        Command::Dnd { state } => match state {
            DndState::On => proxy.set_dnd(true).await?,
            DndState::Off => proxy.set_dnd(false).await?,
            DndState::Toggle => {
                let current = proxy.get_state().await?;
                proxy.set_dnd(!current.dnd_enabled).await?;
            }
        },
        Command::Inhibit { reason, scope } => {
            let token = proxy.inhibit(&reason, scope.as_scope()).await?;
            println!("{token}");
        }
        Command::Uninhibit { id } => {
            proxy.uninhibit(id).await?;
        }
        Command::ListInhibitors => {
            let inhibitors = proxy.list_inhibitors().await?;
            println!("inhibitors: {}", inhibitors.len());
            for (id, reason, scope, owner) in inhibitors {
                println!("- #{id} scope={scope} owner={owner} reason={reason}");
            }
        }
        Command::CssCheck => {
            // Handled before the D-Bus connection is created.
        }
    }

    Ok(())
}

fn print_notifications(label: &str, notifications: &[NotificationView], full: bool) {
    let limit = if full {
        util::diagnostic_log_limit()
    } else {
        util::default_log_limit()
    };
    println!("{} notifications: {}", label, notifications.len());
    for notification in notifications {
        let summary = util::sanitize_log_value(&notification.summary, limit);
        println!(
            "- #{id} [{app}] {summary}",
            id = notification.id,
            app = notification.app_name,
            summary = summary
        );
    }
}

fn follow_debug_logs() -> Result<()> {
    let status = ProcCommand::new("journalctl")
        .args([
            "--user",
            "-f",
            "-u",
            "unixnotis-daemon.service",
            "-o",
            "cat",
        ])
        .status()
        .context("start journalctl follow")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("journalctl exited with status {}", status))
    }
}

fn run_css_check() -> Result<()> {
    gtk::init().context("initialize gtk")?;
    let config_dir = Config::default_config_dir().context("resolve config directory")?;
    let display_root = display_config_root(&config_dir);
    if !config_dir.exists() {
        return Err(anyhow!("config directory not found: {}", display_root));
    }
    if !config_dir.is_dir() {
        return Err(anyhow!("config path is not a directory: {}", display_root));
    }
    let css_files = collect_css_files(&config_dir)?;
    if css_files.is_empty() {
        return Err(anyhow!(
            "no css files found under {} (backup directories are skipped)",
            display_root
        ));
    }

    let error_count = Arc::new(AtomicUsize::new(0));
    let provider = CssProvider::new();
    let error_count_clone = error_count.clone();
    let config_root = config_dir.clone();
    let display_root_clone = display_root.clone();
    provider.connect_parsing_error(move |_provider, section, error| {
        error_count_clone.fetch_add(1, Ordering::Relaxed);
        let location = section.start_location();
        let file = section
            .file()
            .and_then(|file| file.path())
            .map(|path| format_display_path(&config_root, &display_root_clone, &path))
            .unwrap_or_else(|| "<data>".to_string());
        eprintln!(
            "css error: {}:{}:{}: {}",
            file,
            location.lines() + 1,
            location.line_chars() + 1,
            error.message()
        );
    });

    for path in &css_files {
        if !path.exists() {
            error_count.fetch_add(1, Ordering::Relaxed);
            let display_path = format_display_path(&config_dir, &display_root, path);
            eprintln!("css error: {}: file not found", display_path);
            continue;
        }
        if !path.is_file() {
            error_count.fetch_add(1, Ordering::Relaxed);
            let display_path = format_display_path(&config_dir, &display_root, path);
            eprintln!("css error: {}: not a regular file", display_path);
            continue;
        }
        provider.load_from_path(path);
    }

    let errors = error_count.load(Ordering::Relaxed);
    if errors > 0 {
        return Err(anyhow!(
            "css-check found {} error(s) under {}",
            errors,
            display_root
        ));
    }

    // Warnings are advisory only; parsing errors remain fatal because GTK will refuse invalid CSS.
    // This keeps css-check strict about syntax while still surfacing override risks.
    let warnings = lint_css_files(&css_files, &config_dir, &display_root)?;
    if warnings > 0 {
        println!(
            "css-check warnings: {} issue(s) under {}",
            warnings, display_root
        );
    }

    println!(
        "css-check ok: {} file(s) checked under {}",
        css_files.len(),
        display_root
    );
    Ok(())
}

fn collect_css_files(root: &Path) -> Result<Vec<PathBuf>> {
    // Depth-first traversal keeps allocations minimal while visiting all theme files.
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let canonical_root = fs::canonicalize(root)
        .with_context(|| format!("resolve config directory {}", root.display()))?;
    visited.insert(canonical_root.clone());
    let mut stack = vec![root.to_path_buf()];
    let mut results = Vec::new();
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .with_context(|| format!("read config directory {}", dir.display()))?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            let is_dir = if file_type.is_dir() {
                true
            } else if file_type.is_symlink() {
                path.is_dir()
            } else {
                false
            };
            if is_dir {
                if is_backup_dir(&path) {
                    continue;
                }
                if let Ok(canonical) = fs::canonicalize(&path) {
                    // Restrict traversal to the config root even when symlinks are present.
                    if !canonical.starts_with(&canonical_root) {
                        continue;
                    }
                    if !visited.insert(canonical) {
                        continue;
                    }
                }
                stack.push(path);
            } else if is_css_file(&path) {
                results.push(path);
            }
        }
    }
    results.sort();
    Ok(results)
}

fn lint_css_files(files: &[PathBuf], config_dir: &Path, display_root: &str) -> Result<usize> {
    let mut warnings = 0usize;
    for path in files {
        let display_path = format_display_path(config_dir, display_root, path);
        // File contents are needed because GTK only reports parse errors, not override hazards.
        let contents = fs::read_to_string(path)
            .with_context(|| format!("read css file {}", path.display()))?;
        // The linter is intentionally shallow and low-cost; it avoids a full CSS parser.
        let report = lint_css_contents(&contents);
        for warning in report {
            warnings += 1;
            eprintln!("css warning: {}: {}", display_path, warning);
        }
    }
    Ok(warnings)
}

fn lint_css_contents(contents: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    // Strip block comments first so selectors and properties are easier to scan.
    let stripped = strip_css_comments(contents);

    // Duplicate @define-color entries are allowed but usually accidental overrides.
    let mut color_defs: HashMap<String, usize> = HashMap::new();
    for line in stripped.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("@define-color") {
            if let Some(name) = rest.split_whitespace().next() {
                let count = color_defs.entry(name.to_string()).or_insert(0);
                *count += 1;
                if *count > 1 {
                    warnings.push(format!(
                        "duplicate @define-color '{}' (later definition overrides earlier)",
                        name
                    ));
                }
            }
        }
    }

    // Track selectors to flag redefinitions that silently override earlier rules.
    let mut selector_seen: HashMap<String, usize> = HashMap::new();
    let mut cursor = 0usize;
    let bytes = stripped.as_bytes();
    while let Some((selector, block, next)) = next_css_block(bytes, cursor) {
        cursor = next;
        // Collapse whitespace so duplicates match even if formatting differs.
        let selector = normalize_selector(&selector);
        if selector.is_empty() {
            continue;
        }
        // Skip at-rules like @keyframes or @media since their bodies are parsed differently.
        if selector.starts_with('@') {
            continue;
        }
        // Split grouped selectors (".a, .b") so each rule is tracked independently.
        for sel in split_selectors(&selector) {
            if sel.is_empty() {
                continue;
            }
            let count = selector_seen.entry(sel.to_string()).or_insert(0);
            *count += 1;
            if *count > 1 {
                warnings.push(format!(
                    "duplicate selector '{}' (later rules override earlier)",
                    sel
                ));
            }
        }
        warnings.extend(lint_css_properties(&selector, &block));
    }

    warnings
}

fn lint_css_properties(selector: &str, block: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Property duplicates within a selector are almost always accidental overrides.
    for chunk in block.split(';') {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((name, _)) = trimmed.split_once(':') else {
            continue;
        };
        let prop = name.trim();
        if prop.is_empty() {
            continue;
        }
        let inserted = seen.insert(prop.to_string());
        if !inserted {
            warnings.push(format!(
                "duplicate property '{}' in selector '{}' (later value overrides earlier)",
                prop, selector
            ));
        }
    }
    warnings
}

fn strip_css_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_comment = false;
    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && matches!(chars.peek(), Some('/')) {
                chars.next();
                in_comment = false;
            }
            continue;
        }
        if ch == '/' && matches!(chars.peek(), Some('*')) {
            chars.next();
            in_comment = true;
            continue;
        }
        output.push(ch);
    }
    output
}

fn next_css_block(bytes: &[u8], start: usize) -> Option<(String, String, usize)> {
    // A lightweight brace scanner is sufficient for identifying selector blocks.
    // Comments are already stripped, so only string literals need to be respected.
    let mut selector_start = start;
    while selector_start < bytes.len() && bytes[selector_start].is_ascii_whitespace() {
        selector_start += 1;
    }
    let mut i = selector_start;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            if b == quote {
                in_string = None;
            }
            i += 1;
            continue;
        }
        if b == b'"' || b == b'\'' {
            in_string = Some(b);
            i += 1;
            continue;
        }
        if b == b'{' {
            let selector = String::from_utf8_lossy(&bytes[selector_start..i]).to_string();
            // Nested braces can appear in at-rules; track depth to find the matching close.
            let mut depth = 1usize;
            i += 1;
            let block_start = i;
            while i < bytes.len() {
                let b = bytes[i];
                if let Some(quote) = in_string {
                    if b == quote {
                        in_string = None;
                    }
                    i += 1;
                    continue;
                }
                if b == b'"' || b == b'\'' {
                    in_string = Some(b);
                    i += 1;
                    continue;
                }
                if b == b'{' {
                    depth += 1;
                } else if b == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        let block = String::from_utf8_lossy(&bytes[block_start..i]).to_string();
                        return Some((selector, block, i + 1));
                    }
                }
                i += 1;
            }
            break;
        }
        i += 1;
    }
    None
}

fn normalize_selector(selector: &str) -> String {
    // Normalize whitespace so the same selector compares equal across formatting styles.
    selector
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn split_selectors(selector: &str) -> Vec<String> {
    // Split on commas so grouped selectors are checked individually.
    selector
        .split(',')
        .map(|part| part.trim().to_string())
        .collect()
}

fn is_backup_dir(path: &Path) -> bool {
    // Backup directories follow the Backup-YYYY-MM-DD pattern (with optional suffix).
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("Backup-"))
        .unwrap_or(false)
}

fn is_css_file(path: &Path) -> bool {
    // CSS validation only applies to *.css files within the config tree.
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("css"))
        .unwrap_or(false)
}

fn display_config_root(config_dir: &Path) -> String {
    // Prefer stable env-rooted display paths for user-facing output.
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        let trimmed = xdg.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.is_absolute() && config_dir == path.join("unixnotis") {
                return "$XDG_CONFIG_HOME/unixnotis".to_string();
            }
        }
    }
    if let Ok(home) = env::var("HOME") {
        let path = PathBuf::from(home).join(".config").join("unixnotis");
        if config_dir == path {
            return "$HOME/.config/unixnotis".to_string();
        }
    }
    config_dir.display().to_string()
}

fn format_display_path(config_dir: &Path, display_root: &str, path: &Path) -> String {
    // Shorten absolute paths to the config root when possible for cleaner output.
    if let Ok(relative) = path.strip_prefix(config_dir) {
        if relative.as_os_str().is_empty() {
            return display_root.to_string();
        }
        return format!("{}/{}", display_root, relative.display());
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_open_panel_debug_default() {
        // Ensures clap default_missing_value maps --debug to the Info level.
        let args =
            Args::try_parse_from(["noticenterctl", "open-panel", "--debug"]).expect("parse args");
        match args.command {
            Command::OpenPanel { debug } => {
                assert!(matches!(debug, Some(DebugLevelArg::Info)));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_open_panel_debug_value() {
        // Verifies explicit debug values map to the requested verbosity.
        let args = Args::try_parse_from(["noticenterctl", "open-panel", "--debug", "verbose"])
            .expect("parse args");
        match args.command {
            Command::OpenPanel { debug } => {
                assert!(matches!(debug, Some(DebugLevelArg::Verbose)));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_dnd_toggle() {
        // Confirms the value enum accepts the toggle state for DND commands.
        let args = Args::try_parse_from(["noticenterctl", "dnd", "toggle"]).expect("parse args");
        match args.command {
            Command::Dnd { state } => {
                assert!(matches!(state, DndState::Toggle));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn debug_level_arg_into_panel_level() {
        // Validates CLI debug levels map to the matching control plane enum.
        let table = [
            (DebugLevelArg::Critical, PanelDebugLevel::Critical),
            (DebugLevelArg::Warn, PanelDebugLevel::Warn),
            (DebugLevelArg::Info, PanelDebugLevel::Info),
            (DebugLevelArg::Verbose, PanelDebugLevel::Verbose),
        ];
        for (arg, expected) in table {
            let mapped: PanelDebugLevel = arg.into();
            assert_eq!(mapped, expected);
        }
    }

    #[test]
    fn parses_inhibit_default_scope() {
        // Ensures inhibit defaults to the "all" scope when omitted.
        let args = Args::try_parse_from(["noticenterctl", "inhibit", "focus"]).expect("parse args");
        match args.command {
            Command::Inhibit { scope, .. } => {
                assert!(matches!(scope, InhibitScopeArg::All));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_inhibit_popups_scope() {
        // Confirms popups scope is accepted for inhibit calls.
        let args = Args::try_parse_from(["noticenterctl", "inhibit", "focus", "--scope", "popups"])
            .expect("parse args");
        match args.command {
            Command::Inhibit { scope, .. } => {
                assert!(matches!(scope, InhibitScopeArg::Popups));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
