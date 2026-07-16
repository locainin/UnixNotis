use std::path::PathBuf;

use super::{CommandReference, HostSpecificCommandPath, OutsideCommandPath};

#[test]
fn command_path_findings_preserve_slot_command_and_resolved_target() {
    let reference = CommandReference {
        slot: "widgets.volume.get_cmd".to_string(),
        command: "scripts/volume".to_string(),
    };
    let outside = OutsideCommandPath {
        slot: reference.slot.clone(),
        command: reference.command.clone(),
        resolved_path: PathBuf::from("../outside"),
    };
    let host_specific = HostSpecificCommandPath {
        slot: reference.slot.clone(),
        command: reference.command.clone(),
        resolved_path: PathBuf::from("scripts/volume"),
    };

    assert_eq!(outside.slot, reference.slot);
    assert_eq!(outside.command, reference.command);
    assert_eq!(outside.resolved_path, PathBuf::from("../outside"));
    assert_eq!(host_specific.resolved_path, PathBuf::from("scripts/volume"));
}
