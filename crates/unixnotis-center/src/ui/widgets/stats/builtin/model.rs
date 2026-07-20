//! Built-in statistic identity and retained sample state

use std::time::Instant;

#[derive(Clone, Debug)]
pub(in crate::ui::widgets::stats) struct BuiltinStat {
    pub(super) kind: BuiltinStatKind,
    pub(super) state: BuiltinState,
}

#[derive(Clone, Debug)]
pub(super) enum BuiltinStatKind {
    Cpu,
    Memory,
    Load,
    Battery,
    Network { iface: Option<String> },
}

#[derive(Clone, Debug)]
pub(super) enum BuiltinState {
    None,
    Cpu {
        last_total: u64,
        last_idle: u64,
    },
    Network {
        last_rx: u64,
        last_tx: u64,
        last_at: Instant,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::ui::widgets::stats) enum BuiltinStatKey {
    // Every CPU card reads the same procfs source
    Cpu,
    // Every memory card reads the same procfs source
    Memory,
    // Load average is shared across cards too
    Load,
    // Battery cards share one aggregated battery snapshot
    Battery,
    // Network cards only share reads when they target the same interface
    Network { iface: Option<String> },
}

impl BuiltinStat {
    pub(super) fn new(kind: BuiltinStatKind) -> Self {
        let state = match kind {
            BuiltinStatKind::Cpu => BuiltinState::Cpu {
                last_total: 0,
                last_idle: 0,
            },
            BuiltinStatKind::Network { .. } => BuiltinState::Network {
                last_rx: 0,
                last_tx: 0,
                last_at: Instant::now(),
            },
            _ => BuiltinState::None,
        };
        Self { kind, state }
    }

    pub(in crate::ui::widgets::stats) fn key(&self) -> BuiltinStatKey {
        match &self.kind {
            BuiltinStatKind::Cpu => BuiltinStatKey::Cpu,
            BuiltinStatKind::Memory => BuiltinStatKey::Memory,
            BuiltinStatKind::Load => BuiltinStatKey::Load,
            BuiltinStatKind::Battery => BuiltinStatKey::Battery,
            BuiltinStatKind::Network { iface } => BuiltinStatKey::Network {
                iface: iface.clone(),
            },
        }
    }
}
