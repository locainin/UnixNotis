use super::super::*;

#[test]
fn default_dnd_menu_uses_only_right_click_and_keeps_stock_deadlines() {
    assert_eq!(
        default_dnd_menu_triggers(),
        vec![DndMenuTrigger::RightClick]
    );
    assert_eq!(default_dnd_menu_choices().len(), 5);
    assert!(matches!(
        &default_dnd_menu_choices()[0],
        DndMenuChoice::Duration { minutes: 30, .. }
    ));
    assert!(matches!(
        &default_dnd_menu_choices()[3],
        DndMenuChoice::Tomorrow {
            hour: 8,
            minute: 0,
            ..
        }
    ));
    assert!(matches!(
        &default_dnd_menu_choices()[4],
        DndMenuChoice::Indefinite { .. }
    ));
}

#[test]
fn dnd_menu_parses_custom_triggers_and_typed_choices() {
    let panel: PanelConfig = toml::from_str(
        r#"
        dnd_menu_triggers = ["right-click", "keyboard"]

        [[dnd_menu_choices]]
        mode = "duration"
        label = "Focus block"
        minutes = 45

        [[dnd_menu_choices]]
        mode = "tomorrow"
        label = "Tomorrow at lunch"
        hour = 12
        minute = 30

        [[dnd_menu_choices]]
        mode = "indefinite"
        label = "Until disabled"
        "#,
    )
    .expect("custom DND menu should parse");

    assert_eq!(
        panel.dnd_menu_triggers,
        vec![DndMenuTrigger::RightClick, DndMenuTrigger::Keyboard]
    );
    assert_eq!(panel.dnd_menu_choices[0].label(), "Focus block");
    assert!(matches!(
        panel.dnd_menu_choices[1],
        DndMenuChoice::Tomorrow {
            hour: 12,
            minute: 30,
            ..
        }
    ));
}
