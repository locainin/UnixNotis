use std::io;

use super::command::CommandSpec;

#[derive(Clone, Debug)]
pub enum ServiceProbe {
    // Exit-only probes fit managers with exact status commands
    ExitStatus(CommandSpec),
    // Some managers need stdout because exit status means "command worked", not "service runs"
    Stdout {
        command: CommandSpec,
        parser: fn(&str) -> bool,
    },
}

impl ServiceProbe {
    pub(in crate::service_manager) const fn exit_status(command: CommandSpec) -> Self {
        Self::ExitStatus(command)
    }

    pub(in crate::service_manager) fn stdout(
        command: CommandSpec,
        parser: fn(&str) -> bool,
    ) -> Self {
        Self::Stdout { command, parser }
    }

    pub fn evaluate(&self) -> io::Result<bool> {
        match self {
            Self::ExitStatus(command) => {
                // systemd and dinit status commands already encode active state in exit status
                command
                    .to_command()?
                    .status()
                    .map(|status| status.success())
            }
            Self::Stdout { command, parser } => {
                // runit status needs stdout parsing because `sv check` can pass for down state
                let output = command.to_command()?.output()?;
                if !output.status.success() {
                    return Ok(false);
                }
                Ok(parser(&String::from_utf8_lossy(&output.stdout)))
            }
        }
    }
}
