//! Control task construction and bounded command channel ownership

use tokio::sync::mpsc;

use super::model::{UiCommand, UiEvent};

// A bounded channel prevents a stalled bus from growing process memory without limit
const UI_COMMAND_QUEUE_CAPACITY: usize = 64;

pub fn start_control_task(
    runtime: &tokio::runtime::Handle,
    sender: async_channel::Sender<UiEvent>,
) -> mpsc::Sender<UiCommand> {
    let (command_tx, command_rx) = mpsc::channel(UI_COMMAND_QUEUE_CAPACITY);
    runtime.spawn(super::reconnect::run_control_loop(sender, command_rx));
    command_tx
}

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;
