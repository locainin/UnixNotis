use super::{CliAction, CliArgs};

#[test]
fn run_action_preserves_the_selected_installer_arguments() {
    let args = CliArgs {
        service_manager: None,
    };
    let CliAction::Run(actual) = CliAction::Run(args) else {
        unreachable!("constructed run action");
    };

    assert_eq!(actual, args);
}
