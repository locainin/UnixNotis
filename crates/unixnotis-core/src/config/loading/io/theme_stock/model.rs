//! Stock theme migration domain types

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::{DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_WIDGETS_CSS};

use super::super::ThemePaths;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StockThemeLayer {
    Panel,
    Widgets,
    Media,
}

impl StockThemeLayer {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Widgets => "widgets",
            Self::Media => "media",
        }
    }

    pub(super) fn path(self, paths: &ThemePaths) -> &Path {
        match self {
            Self::Panel => &paths.panel_css,
            Self::Widgets => &paths.widgets_css,
            Self::Media => &paths.media_css,
        }
    }

    pub(super) fn set_path(self, paths: &mut ThemePaths, path: PathBuf) {
        match self {
            Self::Panel => paths.panel_css = path,
            Self::Widgets => paths.widgets_css = path,
            Self::Media => paths.media_css = path,
        }
    }

    pub(super) const fn current_contents(self) -> &'static [u8] {
        match self {
            Self::Panel => DEFAULT_PANEL_CSS.as_bytes(),
            Self::Widgets => DEFAULT_WIDGETS_CSS.as_bytes(),
            Self::Media => DEFAULT_MEDIA_CSS.as_bytes(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileSnapshot {
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) size: u64,
    pub(super) modified: SystemTime,
    pub(super) digest: blake3::Hash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StockThemeCandidate {
    pub(super) layer: StockThemeLayer,
    pub(super) path: PathBuf,
    pub(super) snapshot: FileSnapshot,
    pub(super) original_contents: Vec<u8>,
}

/// Exact known stock files that can be previewed or updated with explicit approval
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockThemeMigration {
    pub(super) candidates: Vec<StockThemeCandidate>,
    pub(super) fingerprint: String,
}

impl StockThemeMigration {
    /// Return how many independently editable theme layers are eligible
    #[must_use]
    pub const fn layer_count(&self) -> usize {
        self.candidates.len()
    }

    /// Return a compact normal-user description of the eligible layers
    #[must_use]
    pub fn layer_summary(&self) -> String {
        let labels = self
            .candidates
            .iter()
            .map(|candidate| candidate.layer.label())
            .collect::<Vec<_>>();
        labels.join(", ")
    }

    /// Return the stable identity used to reject stale UI actions
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Result of an approved stock theme update
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockThemeApplyReport {
    /// Number of active theme files replaced after revalidation
    pub updated_layers: usize,
}
