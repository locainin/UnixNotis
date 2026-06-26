use std::process::{Command, Stdio};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    // Human-readable command shown in logs without exposing inherited environment values
    label: String,
    // Executable name stays separate so tests can assert command construction directly
    program: String,
    // Arguments are stored as data so no shell parsing is involved
    args: Vec<String>,
    // Env overrides keep sensitive values out of argv while still giving child tools the session
    envs: Vec<(String, String)>,
    // Some probes are intentionally quiet to avoid corrupting the TUI
    suppress_stdout: bool,
    suppress_stderr: bool,
}

impl CommandSpec {
    pub(super) fn new<I, S>(label: impl Into<String>, program: impl Into<String>, args: I) -> Self
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

    pub(super) fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        // Values live in the child environment instead of the process argument list
        self.envs.push((name.into(), value.into()));
        self
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

    #[cfg(test)]
    pub fn envs(&self) -> &[(String, String)] {
        &self.envs
    }

    pub fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
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
        command
    }
}
