use std::path::Path;

use super::super::css::*;
use unixnotis_core::Config;

#[test]
fn theme_path_check_always_precedes_the_environment_dependent_css_check() {
    let checks = inspect_css(Path::new("/tmp/unixnotis/config.toml"), &Config::default());

    assert!(!checks.is_empty());
    assert_eq!(checks[0].id, "css.theme-paths");
}
