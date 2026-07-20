use std::process::{Command, Stdio};
use unixnotis_core::CommandSpec as ProcessCommandSpec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    // Human-readable command shown in logs without exposing inherited environment values
    label: String,
    // Shared process spec keeps executable, arguments, and environment structurally separate
    command: ProcessCommandSpec,
    // Some probes are intentionally quiet to avoid corrupting the TUI
    suppress_stdout: bool,
    suppress_stderr: bool,
}

impl CommandSpec {
    pub(in crate::service_manager) fn new<I, S>(
        label: impl Into<String>,
        program: impl Into<String>,
        args: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        Self {
            label: label.into(),
            command: ProcessCommandSpec::direct(
                program.into(),
                args.into_iter().map(|arg| arg.to_string()),
            ),
            suppress_stdout: false,
            suppress_stderr: false,
        }
    }

    pub(in crate::service_manager) fn env(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        // Values live in the child environment instead of the process argument list
        self.command = self.command.with_env(name.into(), value.into());
        self
    }

    pub(in crate::service_manager) const fn quiet(mut self) -> Self {
        // Availability probes should not leak command output into the parent process
        self.suppress_stdout = true;
        self.suppress_stderr = true;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn program(&self) -> &str {
        self.command
            .program()
            .and_then(|program| program.to_str())
            .expect("installer service commands always use UTF-8 direct programs")
    }

    #[cfg(test)]
    pub fn args(&self) -> Vec<String> {
        self.command
            .args()
            .unwrap_or_default()
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }

    #[cfg(test)]
    pub fn envs(&self) -> Vec<(String, String)> {
        self.command
            .env()
            .expect("installer service commands are always direct")
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect()
    }

    pub fn to_command(&self) -> std::io::Result<Command> {
        let program = self.program();
        let mut command = Command::new(super::command_routing::command_program(program)?);
        // CommandSpec never goes through a shell, which keeps service-manager commands predictable
        command.args(self.command.args().unwrap_or_default());
        command.envs(self.command.env().into_iter().flatten());
        if self.suppress_stdout {
            command.stdout(Stdio::null());
        }
        if self.suppress_stderr {
            command.stderr(Stdio::null());
        }
        Ok(command)
    }
}
