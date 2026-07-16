//! Bounded and backend-aware doctor log acquisition

mod routing;
mod sanitize;
mod systemd;

pub(super) use routing::collect_logs;

#[cfg(test)]
mod tests;
