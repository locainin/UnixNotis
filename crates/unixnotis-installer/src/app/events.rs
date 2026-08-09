//! Event types used to coordinate the installer UI and worker thread.

use crossterm::event::Event;
use std::path::PathBuf;

pub enum ExitAction {
    // Return from the installer without launching follow-up work
    None,
    // Restore the terminal before starting a trial from this repository
    RunTrial { repo_root: PathBuf },
}

pub enum UiMessage {
    Input(Event),
    ReleaseStatus(crate::release::ReleaseStatus),
    Worker(WorkerEvent),
}

pub enum WorkerEvent {
    StepStarted(usize),
    StepCompleted(usize),
    StepFailed {
        index: usize,
        // The summary stays short enough for the progress header
        summary: String,
        // The complete anyhow chain stays in the bounded log view
        detail: String,
    },
    RecoveryRequired {
        index: usize,
        // The summary stays short enough for the progress header
        summary: String,
        // The complete anyhow chain stays in the bounded log view
        detail: String,
    },
    LogLine(String),
    Finished,
}

#[cfg(test)]
#[path = "tests/events.rs"]
mod tests;
