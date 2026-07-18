use super::{parse_args, test_support, CliAction};

#[test]
fn empty_argument_list_starts_the_installer_with_default_options() {
    let parsed = parse_args(test_support::args(&[])).expect("empty arguments");
    let CliAction::Run(args) = parsed else {
        panic!("expected run action");
    };

    assert!(args.service_manager.is_none());
}
