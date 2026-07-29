//! Non-mutating custom theme compatibility flow

use gio::prelude::FileExt;
use tracing::warn;
use unixnotis_core::{persist_theme_mode, ThemeMode};

use crate::ui::reload::ReloadNoticeKind;
use crate::ui::UiState;

impl UiState {
    pub(in crate::ui) fn initialize_theme_compatibility(&mut self) {
        self.refresh_theme_compatibility_notice();
    }

    pub(in crate::ui) fn refresh_theme_compatibility_notice(&mut self) {
        let state = self.css.theme_contract();
        if state.is_incompatible() {
            self.set_reload_notice(
                ReloadNoticeKind::ThemeCompatibility,
                "Theme is incompatible with this UnixNotis version.\nYour files were not changed; embedded stock styling is active.",
                false,
                &format!("{state:?}"),
            );
        } else {
            self.clear_reload_notice(ReloadNoticeKind::ThemeCompatibility);
        }
    }

    pub(in crate::ui) fn use_stock_theme(&mut self) {
        if let Err(error) = persist_theme_mode(&self.config_path, ThemeMode::Stock) {
            warn!(
                kind = error.kind(),
                "failed to persist the embedded stock theme selection"
            );
            self.set_reload_notice(
                ReloadNoticeKind::ThemeCompatibility,
                "Could not save the stock theme selection.\nYour custom files were not changed.",
                true,
                error.kind(),
            );
            return;
        }

        // Reloading from disk makes the saved choice and active providers advance together
        let _outcome = self.reload_config();
    }

    pub(in crate::ui) fn open_theme_folder(&self) {
        let folder = gio::File::for_path(&self.css.theme_paths().base_dir);
        let uri = folder.uri();
        if let Err(error) =
            gio::AppInfo::launch_default_for_uri(uri.as_str(), None::<&gio::AppLaunchContext>)
        {
            warn!(?error, "failed to open the configured theme folder");
        }
    }
}
