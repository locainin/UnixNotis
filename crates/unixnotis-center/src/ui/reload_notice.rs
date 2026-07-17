//! Priority and dismissal state for configuration and CSS reload notices

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReloadNoticeKind {
    Config,
    Css,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReloadNoticeFingerprint {
    pub(super) kind: ReloadNoticeKind,
    pub(super) identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReloadNotice {
    pub(super) fingerprint: ReloadNoticeFingerprint,
    pub(super) message: String,
    pub(super) error: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ReloadNoticeState {
    config: Option<ReloadNotice>,
    css: Option<ReloadNotice>,
    dismissed_config: Option<ReloadNoticeFingerprint>,
    dismissed_css: Option<ReloadNoticeFingerprint>,
}

impl ReloadNoticeState {
    pub(super) fn set(&mut self, notice: ReloadNotice) {
        match notice.fingerprint.kind {
            ReloadNoticeKind::Config => {
                // A distinct failure starts a fresh dismissal lifecycle
                Self::replace_notice(&mut self.config, &mut self.dismissed_config, notice);
            }
            ReloadNoticeKind::Css => {
                // Duplicate watcher events retain dismissal for the same failure
                Self::replace_notice(&mut self.css, &mut self.dismissed_css, notice);
            }
        }
    }

    pub(super) fn clear(&mut self, kind: ReloadNoticeKind) {
        // Recovery ends both the active failure and its dismissal lifecycle
        match kind {
            ReloadNoticeKind::Config => {
                self.config = None;
                self.dismissed_config = None;
            }
            ReloadNoticeKind::Css => {
                self.css = None;
                self.dismissed_css = None;
            }
        }
    }

    fn replace_notice(
        active: &mut Option<ReloadNotice>,
        dismissed: &mut Option<ReloadNoticeFingerprint>,
        notice: ReloadNotice,
    ) {
        let same_failure = active
            .as_ref()
            .is_some_and(|current| current.fingerprint == notice.fingerprint);
        if !same_failure {
            // An intervening fingerprint makes every older dismissal stale
            *dismissed = None;
        }
        *active = Some(notice);
    }

    pub(super) fn dismiss_visible(&mut self) {
        // Dismissal stores the fingerprint rather than discarding the underlying failure
        let Some(notice) = self.visible().cloned() else {
            return;
        };
        match notice.fingerprint.kind {
            // Each class remembers dismissal independently across priority changes
            ReloadNoticeKind::Config => self.dismissed_config = Some(notice.fingerprint),
            ReloadNoticeKind::Css => self.dismissed_css = Some(notice.fingerprint),
        }
    }

    pub(super) fn visible(&self) -> Option<&ReloadNotice> {
        // Rejected configuration always outranks a recoverable CSS fallback warning
        self.config
            .as_ref()
            .filter(|notice| self.dismissed_config.as_ref() != Some(&notice.fingerprint))
            .or_else(|| {
                // CSS becomes visible again when no undismissed config failure remains
                self.css
                    .as_ref()
                    .filter(|notice| self.dismissed_css.as_ref() != Some(&notice.fingerprint))
            })
    }
}

#[cfg(test)]
#[path = "tests/reload_notice.rs"]
mod tests;
