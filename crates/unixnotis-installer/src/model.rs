//! Shared installer types for action selection and progress reporting

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionMode {
    Test,
    Install,
    Uninstall,
    Reset,
}

impl ActionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Test => "Trial run",
            Self::Install => "Install",
            Self::Uninstall => "Uninstall",
            Self::Reset => "Reset config",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResetAction {
    ResetDefaults,
    RestoreBackup { path: std::path::PathBuf },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
}

pub struct ActionStep {
    pub name: &'static str,
    pub status: StepStatus,
}
