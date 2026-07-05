//! Control-plane D-Bus command execution

mod client;
mod dispatch;
mod output_gate;

pub(crate) use dispatch::handle_command;

#[cfg(test)]
mod tests;
