//! D-Bus event publication after committed daemon mutations

mod inhibitors;
mod notifications;
mod publisher;
mod state;

pub(in crate::daemon) use publisher::DaemonEventPublisher;

#[cfg(test)]
mod tests;
