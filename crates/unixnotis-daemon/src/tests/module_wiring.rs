use super::child_process::{spawn_center_supervisor, spawn_popups_supervisor};
use super::sound::SoundSettings;
use super::trial_mode::{prepare_trial, restore_previous, TrialState};

#[test]
fn explicit_root_modules_keep_expected_parent_exports() {
    // The daemon uses explicit root.rs files instead of code-bearing mod.rs files
    // Referencing the parent-facing items catches stale child-module paths after renames
    let center = spawn_center_supervisor;
    let popups = spawn_popups_supervisor;
    let trial = prepare_trial;
    let restore = restore_previous;

    assert_eq!(
        std::any::type_name_of_val(&center),
        std::any::type_name_of_val(&spawn_center_supervisor)
    );
    assert_eq!(
        std::any::type_name_of_val(&popups),
        std::any::type_name_of_val(&spawn_popups_supervisor)
    );
    assert_eq!(
        std::any::type_name_of_val(&trial),
        std::any::type_name_of_val(&prepare_trial)
    );
    assert_eq!(
        std::any::type_name_of_val(&restore),
        std::any::type_name_of_val(&restore_previous)
    );
    assert!(std::mem::size_of::<TrialState>() > 0);
    assert!(std::mem::size_of::<SoundSettings>() > 0);
}
