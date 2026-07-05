use clap::Parser;

use super::Command;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct Args {
    // Subcommands map 1:1 to the daemon control surface
    #[command(subcommand)]
    pub(crate) command: Command,
}
