//! Export containment and host-specific script checks

mod scripts;
mod theme;

pub(in crate::preset::export) use scripts::{
    capture_file_overrides, restore_file_overrides, rewrite_host_specific_script_paths_in_sources,
    HostSpecificScriptLeak,
};
pub(in crate::preset::export) use theme::validate_theme_paths_stay_in_root;
