//! Explicit direct and shell command representations

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One command whose shell boundary is selected by configuration, not inferred at runtime
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CommandSpec {
    /// Executes one program with literal arguments and child-local environment overrides
    Direct {
        program: PathBuf,
        #[serde(default, with = "os_string_vec")]
        args: Vec<OsString>,
        #[serde(default, with = "os_string_map")]
        env: BTreeMap<OsString, OsString>,
    },
    /// Executes one script through the system's POSIX shell
    Shell { script: String },
}

impl fmt::Display for CommandSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display_lossy())
    }
}

impl CommandSpec {
    /// Build a direct command without any shell parsing or expansion
    pub fn direct<I, S>(program: impl Into<PathBuf>, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self::Direct {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            env: BTreeMap::new(),
        }
    }

    /// Build an explicit POSIX shell command
    pub fn shell(script: impl Into<String>) -> Self {
        Self::Shell {
            script: script.into(),
        }
    }

    /// Add one child-local environment value to a direct command
    #[must_use]
    pub fn with_env(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        if let Self::Direct { env, .. } = &mut self {
            env.insert(name.into(), value.into());
        }
        self
    }

    #[must_use]
    pub const fn is_shell(&self) -> bool {
        matches!(self, Self::Shell { .. })
    }

    #[must_use]
    /// Reports whether the command evaluates an inline shell command string
    pub fn uses_shell_command_string(&self) -> bool {
        match self {
            Self::Shell { .. } => true,
            Self::Direct { program, args, .. } => {
                // Basenames keep absolute interpreter paths and PATH lookups equivalent
                let Some(shell) = program
                    .file_name()
                    .and_then(OsStr::to_str)
                    .filter(|name| is_shell_program(name))
                else {
                    return false;
                };

                shell_args_use_command_string(shell, args)
            }
        }
    }

    /// Compatibility alias for the former inline shell command detector
    #[deprecated(since = "1.2.0", note = "use uses_shell_command_string")]
    #[must_use]
    pub fn invokes_shell(&self) -> bool {
        self.uses_shell_command_string()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Direct { program, .. } => program.as_os_str().is_empty(),
            Self::Shell { script } => script.trim().is_empty(),
        }
    }

    #[must_use]
    pub fn program(&self) -> Option<&Path> {
        match self {
            Self::Direct { program, .. } => Some(program),
            Self::Shell { .. } => None,
        }
    }

    #[must_use]
    pub fn args(&self) -> Option<&[OsString]> {
        match self {
            Self::Direct { args, .. } => Some(args),
            Self::Shell { .. } => None,
        }
    }

    #[must_use]
    pub const fn env(&self) -> Option<&BTreeMap<OsString, OsString>> {
        match self {
            Self::Direct { env, .. } => Some(env),
            Self::Shell { .. } => None,
        }
    }

    #[must_use]
    pub fn script(&self) -> Option<&str> {
        match self {
            Self::Direct { .. } => None,
            Self::Shell { script } => Some(script),
        }
    }

    /// Replace a runtime placeholder without reparsing direct arguments
    #[must_use]
    pub fn replace(&self, placeholder: &str, value: &str) -> Self {
        match self {
            Self::Direct { program, args, env } => Self::Direct {
                program: replace_os(program.as_os_str(), placeholder, value).into(),
                args: args
                    .iter()
                    .map(|arg| replace_os(arg, placeholder, value))
                    .collect(),
                env: env
                    .iter()
                    .map(|(name, current)| {
                        (
                            name.clone(),
                            replace_os(current.as_os_str(), placeholder, value),
                        )
                    })
                    .collect(),
            },
            Self::Shell { script } => Self::shell(script.replace(placeholder, value)),
        }
    }

    /// Produce bounded-log input without changing execution semantics
    #[must_use]
    pub fn display_lossy(&self) -> String {
        match self {
            Self::Direct { program, args, .. } => {
                let mut parts = Vec::with_capacity(args.len() + 1);
                parts.push(program.as_os_str().to_string_lossy().into_owned());
                parts.extend(args.iter().map(|arg| arg.to_string_lossy().into_owned()));
                parts.join(" ")
            }
            Self::Shell { script } => script.clone(),
        }
    }
}

fn is_shell_program(name: &str) -> bool {
    matches!(
        name,
        "sh" | "ash" | "bash" | "dash" | "fish" | "ksh" | "zsh"
    )
}

fn shell_args_use_command_string(shell: &str, args: &[OsString]) -> bool {
    let mut option_value_pending = false;
    for argument in args {
        let Some(argument) = argument.to_str() else {
            // TOML arguments are UTF-8, while an opaque programmatic operand ends option parsing
            return false;
        };
        if option_value_pending {
            option_value_pending = false;
            continue;
        }
        if matches!(argument, "-" | "--") {
            // Both portable terminators make every following `-c` literal script data
            return false;
        }
        if is_command_string_flag(argument)
            || (shell == "fish" && is_fish_command_string_option(argument))
        {
            return true;
        }
        if !argument.starts_with(['-', '+']) {
            // The first positional operand is the script path for direct shell execution
            return false;
        }
        option_value_pending = shell_option_takes_next_value(shell, argument);
    }
    false
}

fn is_command_string_flag(argument: &str) -> bool {
    argument.strip_prefix('-').is_some_and(|flags| {
        // Long options such as `--norc` contain a letter c but do not evaluate command text
        !flags.starts_with('-') && flags.contains('c')
    })
}

fn is_fish_command_string_option(argument: &str) -> bool {
    // Fish documents a long spelling in addition to the shared short `-c` form
    argument == "--command" || argument.starts_with("--command=")
}

fn shell_option_takes_next_value(shell: &str, argument: &str) -> bool {
    match shell {
        // Bash accepts option names and startup files as separate operands before `-c`
        "bash" => matches!(
            argument,
            "-o" | "+o" | "-O" | "+O" | "--init-file" | "--rcfile"
        ),
        // These POSIX-style shells accept a separate value for `-o`
        "sh" | "ash" | "dash" => argument == "-o",
        "ksh" => matches!(argument, "-o" | "+o" | "-R"),
        "zsh" => matches!(argument, "-o" | "+o"),
        // Fish accepts both short and long value-taking startup options
        "fish" => matches!(
            argument,
            "-C" | "-d"
                | "-o"
                | "-p"
                | "-f"
                | "--init-command"
                | "--debug"
                | "--debug-output"
                | "--profile"
                | "--profile-startup"
                | "--features"
        ),
        _ => false,
    }
}

fn replace_os(value: &OsStr, placeholder: &str, replacement: &str) -> OsString {
    // TOML-originated values are UTF-8; non-UTF-8 programmatic values remain byte-for-byte stable
    value.to_str().map_or_else(
        || value.to_os_string(),
        |value| OsString::from(value.replace(placeholder, replacement)),
    )
}

mod os_string_vec {
    use std::ffi::OsString;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S>(values: &[OsString], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        values
            .iter()
            .map(|value| {
                value
                    .to_str()
                    .ok_or_else(|| serde::ser::Error::custom("command argument is not UTF-8"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<OsString>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)
            .map(|values| values.into_iter().map(OsString::from).collect())
    }
}

mod os_string_map {
    use std::collections::BTreeMap;
    use std::ffi::OsString;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S>(
        values: &BTreeMap<OsString, OsString>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let values = values
            .iter()
            .map(|(name, value)| {
                let name = name
                    .to_str()
                    .ok_or_else(|| serde::ser::Error::custom("environment name is not UTF-8"))?;
                let value = value
                    .to_str()
                    .ok_or_else(|| serde::ser::Error::custom("environment value is not UTF-8"))?;
                Ok((name, value))
            })
            .collect::<Result<BTreeMap<_, _>, S::Error>>()?;
        values.serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<OsString, OsString>, D::Error>
    where
        D: Deserializer<'de>,
    {
        BTreeMap::<String, String>::deserialize(deserializer).map(|values| {
            values
                .into_iter()
                .map(|(name, value)| (OsString::from(name), OsString::from(value)))
                .collect()
        })
    }
}
