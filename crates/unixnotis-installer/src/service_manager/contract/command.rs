use std::process::{Command, Stdio};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    // Human-readable command shown in logs without exposing inherited environment values
    label: String,
    // Executable name stays separate so tests can assert command construction directly
    program: String,
    // Arguments are stored as data so no shell parsing is involved
    pub(in crate::service_manager::contract) args: Vec<String>,
    // Env overrides keep sensitive values out of argv while still giving child tools the session
    pub(in crate::service_manager::contract) envs: Vec<(String, String)>,
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
            program: program.into(),
            args: args.into_iter().map(|arg| arg.to_string()).collect(),
            envs: Vec::new(),
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
        self.envs.push((name.into(), value.into()));
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
        &self.program
    }

    pub fn to_command(&self) -> std::io::Result<Command> {
        let mut command = Command::new(super::command_routing::command_program(&self.program)?);
        // CommandSpec never goes through a shell, which keeps service-manager commands predictable
        command.args(&self.args);
        for (name, value) in &self.envs {
            // Only backend-selected variables are added; inherited process env is left alone
            command.env(name, value);
        }
        if self.suppress_stdout {
            command.stdout(Stdio::null());
        }
        if self.suppress_stderr {
            command.stderr(Stdio::null());
        }
        Ok(command)
    }
}
