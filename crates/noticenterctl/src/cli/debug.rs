use clap::ValueEnum;
use unixnotis_core::PanelDebugLevel;

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum DebugLevelArg {
    // Only critical diagnostic output
    Critical,
    // Warnings and above
    Warn,
    // Informational output
    Info,
    // Verbose diagnostics
    Verbose,
}

impl From<DebugLevelArg> for PanelDebugLevel {
    fn from(value: DebugLevelArg) -> Self {
        match value {
            DebugLevelArg::Critical => PanelDebugLevel::Critical,
            DebugLevelArg::Warn => PanelDebugLevel::Warn,
            DebugLevelArg::Info => PanelDebugLevel::Info,
            DebugLevelArg::Verbose => PanelDebugLevel::Verbose,
        }
    }
}
