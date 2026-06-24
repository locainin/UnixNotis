//! Managed Hyprland bootstrap block rendering and stripping

use std::path::Path;

use crate::paths::format_with_home;

use super::super::{log_line, ActionContext};

pub(in crate::actions::hyprland) const HYPR_BOOTSTRAP_START: &str =
    "# BEGIN UNIXNOTIS SESSION BOOTSTRAP";
pub(in crate::actions::hyprland) const HYPR_BOOTSTRAP_END: &str =
    "# END UNIXNOTIS SESSION BOOTSTRAP";

pub(in crate::actions::hyprland) struct HyprlandStripResult {
    pub(in crate::actions::hyprland) stripped: String,
    pub(in crate::actions::hyprland) block_found: bool,
    pub(in crate::actions::hyprland) malformed: bool,
}

pub(in crate::actions::hyprland) fn render_hyprland_bootstrap_block(lines: &[String]) -> String {
    // Block markers let uninstall remove only installer-owned content
    let mut block = String::new();
    block.push_str(HYPR_BOOTSTRAP_START);
    block.push('\n');
    block.push_str("# UnixNotis session bootstrap\n");
    block.push_str("# Ensures systemd user environment carries Wayland session variables\n");
    block.push_str("# Managed by unixnotis-installer; safe to remove with uninstall\n");
    for line in lines {
        block.push_str(line);
        block.push('\n');
    }
    block.push_str(HYPR_BOOTSTRAP_END);
    block.push('\n');
    block
}

pub(in crate::actions::hyprland) fn strip_hyprland_bootstrap_block(
    ctx: &mut ActionContext,
    contents: &str,
    hypr_config: &Path,
) -> HyprlandStripResult {
    let mut remaining = contents.to_string();
    let mut block_found = false;

    while let Some(start) = remaining.find(HYPR_BOOTSTRAP_START) {
        let Some(end_rel) = remaining[start..].find(HYPR_BOOTSTRAP_END) else {
            return HyprlandStripResult {
                stripped: remaining,
                block_found: false,
                malformed: true,
            };
        };
        let end = start + end_rel + HYPR_BOOTSTRAP_END.len();
        let after_end = if remaining[end..].starts_with('\n') {
            end + 1
        } else {
            end
        };
        remaining.replace_range(start..after_end, "");
        block_found = true;
    }

    if contents.contains(HYPR_BOOTSTRAP_END) && !contents.contains(HYPR_BOOTSTRAP_START) {
        log_line(
            ctx,
            format!(
                "Warning: malformed UnixNotis bootstrap block in {}; dangling end marker",
                format_with_home(hypr_config)
            ),
        );
        return HyprlandStripResult {
            stripped: contents.to_string(),
            block_found: false,
            malformed: true,
        };
    }

    HyprlandStripResult {
        stripped: remaining,
        block_found,
        malformed: false,
    }
}
