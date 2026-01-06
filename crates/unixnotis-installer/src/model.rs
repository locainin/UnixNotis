//! Shared installer types for action selection and progress reporting.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionMode {
    Test,
    Install,
    Uninstall,
    Reset,
}

impl ActionMode {
    pub fn label(self) -> &'static str {
        match self {
            ActionMode::Test => "Trial run",
            ActionMode::Install => "Install",
            ActionMode::Uninstall => "Uninstall",
            ActionMode::Reset => "Reset config",
        }
    }
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
