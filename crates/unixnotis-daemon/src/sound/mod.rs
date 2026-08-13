//! Notification sound settings and playback backends

mod backend;
mod command;
mod resolve;
mod settings;
mod source;
mod wav;

pub use settings::SoundSettings;
use source::{SoundFile, SoundSource};
