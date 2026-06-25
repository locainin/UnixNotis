use std::process::{Command, Stdio};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    // Human-readable command shown in logs without exposing inherited environment values
    label: String,
    // Executable name stays separate so tests can assert command construction directly
    program: String,
    // Arguments are stored as data so no shell parsing is involved
    args: Vec<String>,
    // Some probes are intentionally quiet to avoid corrupting the TUI
    suppress_stdout: bool,
    suppress_stderr: bool,
}

impl CommandSpec {
    pub(super) fn new(label: impl Into<String>, program: impl Into<String>, args: &[&str]) -> Self {
        Self {
            label: label.into(),
            program: program.into(),
            args: args.iter().map(|arg| (*arg).to_string()).collect(),
            suppress_stdout: false,
            suppress_stderr: false,
        }
    }

    pub(super) fn quiet(mut self) -> Self {
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

    #[cfg(test)]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        // CommandSpec never goes through a shell, which keeps service-manager commands predictable
        command.args(&self.args);
        if self.suppress_stdout {
            command.stdout(Stdio::null());
        }
        if self.suppress_stderr {
            command.stderr(Stdio::null());
        }
        command
    }
}
