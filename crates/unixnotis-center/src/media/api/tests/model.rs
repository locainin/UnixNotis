use super::super::MediaCommand;

#[test]
fn media_command_keeps_the_target_player_name() {
    let command = MediaCommand::Next {
        bus_name: "org.mpris.MediaPlayer2.test".to_string(),
    };

    let MediaCommand::Next { bus_name } = command else {
        panic!("expected next command");
    };
    assert_eq!(bus_name, "org.mpris.MediaPlayer2.test");
}
