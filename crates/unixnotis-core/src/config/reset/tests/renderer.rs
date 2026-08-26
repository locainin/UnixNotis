use super::super::render_default_config_toml;
use crate::Config;

#[test]
fn reset_uses_the_same_annotated_default_renderer() {
    let rendered = render_default_config_toml(&Config::default()).expect("render defaults");
    assert!(rendered.contains("# Exact pixel height override"));
    assert!(rendered.contains("# Disable panel animation"));
    assert!(rendered.contains("pause_on_hover = true"));
}
