//! Event types used to coordinate the installer UI and worker thread.

use crossterm::event::Event;

pub enum UiMessage {
    Input(Event),
    Worker(WorkerEvent),
}

pub enum WorkerEvent {
    StepStarted(usize),
    StepCompleted(usize),
    StepFailed(usize, String),
    LogLine(String),
    Finished,
}
