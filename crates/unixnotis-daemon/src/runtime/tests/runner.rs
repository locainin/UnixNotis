use clap::Parser;

use super::trial_requested;
use crate::cli::Args;

#[test]
fn trial_preparation_is_enabled_only_by_the_trial_flag() {
    let normal = Args::try_parse_from(["unixnotis-daemon"]).expect("parse normal daemon command");
    let trial = Args::try_parse_from(["unixnotis-daemon", "--trial", "--yes"])
        .expect("parse trial daemon command");

    assert!(!trial_requested(&normal));
    assert!(trial_requested(&trial));
}
