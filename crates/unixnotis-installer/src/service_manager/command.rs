use std::process::{Command, Stdio};

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::path::{Path, PathBuf};

#[cfg(test)]
thread_local! {
    // Flow tests run real installer code with fake manager binaries
    // Thread-local routing avoids changing PATH for the whole parallel test process
    static FAKE_COMMAND_BIN: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

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
        let mut command = Command::new(command_program(&self.program));
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

fn command_program(program: &str) -> std::ffi::OsString {
    #[cfg(test)]
    if let Some(fake_program) = fake_command_program(program) {
        // Test-only fake routing keeps CommandSpec execution deterministic under cargo test
        return fake_program.into_os_string();
    }

    // Production always executes the program name exactly as the backend provided it
    program.into()
}

#[cfg(test)]
fn fake_command_program(program: &str) -> Option<PathBuf> {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }
    FAKE_COMMAND_BIN.with(|fake_bin| {
        let fake_bin = fake_bin.borrow();
        let candidate = fake_bin.as_ref()?.join(program);
        // Only route commands that the test explicitly provided
        candidate.is_file().then_some(candidate)
    })
}

#[cfg(test)]
pub(crate) struct FakeCommandBinGuard {
    // Previous routing supports nested helpers without leaking fake bins across tests
    previous: Option<PathBuf>,
}

#[cfg(test)]
impl Drop for FakeCommandBinGuard {
    fn drop(&mut self) {
        FAKE_COMMAND_BIN.with(|fake_bin| {
            // Restore the previous test override so nested helpers stay scoped
            *fake_bin.borrow_mut() = self.previous.take();
        });
    }
}

#[cfg(test)]
pub(crate) fn use_fake_command_bin(path: &Path) -> FakeCommandBinGuard {
    FAKE_COMMAND_BIN.with(|fake_bin| {
        let mut fake_bin = fake_bin.borrow_mut();
        // replace() returns the old route so Drop can restore it exactly
        let previous = fake_bin.replace(path.to_path_buf());
        FakeCommandBinGuard { previous }
    })
}
