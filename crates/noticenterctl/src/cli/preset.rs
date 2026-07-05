use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub(crate) enum PresetCommand {
    // Export the current config tree into one shareable bundle file
    Export {
        output: String,
        #[arg(long = "except", value_name = "PATH")]
        except: Vec<String>,
        #[arg(long)]
        force: bool,
    },
    // Import a bundle into the current config tree
    Import {
        input: String,
        #[arg(long = "except", value_name = "PATH")]
        except: Vec<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        allow_exec: bool,
    },
    // Print bundle metadata and included files without writing anything
    Inspect {
        input: String,
    },
}
