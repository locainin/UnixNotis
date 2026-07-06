use super::*;

#[derive(serde::Deserialize)]
struct SectionOrderFixture {
    sections: Vec<PanelSection>,
    widgets: Vec<PanelWidgetSection>,
}

#[test]
fn default_panel_section_order_keeps_widgets_above_notifications() {
    assert_eq!(
        default_panel_section_order(),
        vec![PanelSection::Widgets, PanelSection::Notifications]
    );
}

#[test]
fn default_panel_widget_order_keeps_sliders_first() {
    assert_eq!(
        default_panel_widget_order(),
        vec![
            PanelWidgetSection::Sliders,
            PanelWidgetSection::Media,
            PanelWidgetSection::Toggles,
            PanelWidgetSection::Stats,
            PanelWidgetSection::Cards,
        ]
    );
}

#[test]
fn panel_sections_parse_from_kebab_case_config_values() {
    let fixture: SectionOrderFixture = toml::from_str(
        r#"
        sections = ["widgets", "notifications"]
        widgets = ["media", "toggles", "sliders", "stats", "cards"]
        "#,
    )
    .expect("sections should parse");

    assert_eq!(
        fixture.sections,
        vec![PanelSection::Widgets, PanelSection::Notifications]
    );
    assert_eq!(
        fixture.widgets,
        vec![
            PanelWidgetSection::Media,
            PanelWidgetSection::Toggles,
            PanelWidgetSection::Sliders,
            PanelWidgetSection::Stats,
            PanelWidgetSection::Cards,
        ]
    );
}
