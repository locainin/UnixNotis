//! Service artifact refresh plans for backend-specific reload work

use std::path::{Path, PathBuf};

use super::CommandSpec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceArtifactRefresh {
    // Simple managers expose one safe command, such as systemd daemon-reload
    Command(CommandSpec),
    // s6 needs a compiled database plus a live database update
    S6Database(S6DatabaseRefresh),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct S6DatabaseRefresh {
    // Root containing sv/ service source dirs and rc/ compiled databases
    data_root: PathBuf,
    // Live tree used by s6-rc and s6-rc-update when supervision is running
    live_root: PathBuf,
}

impl S6DatabaseRefresh {
    pub(super) fn new(data_root: PathBuf, live_root: PathBuf) -> Self {
        Self {
            data_root,
            live_root,
        }
    }

    pub fn live_root(&self) -> &Path {
        &self.live_root
    }

    pub fn source_root(&self) -> PathBuf {
        // s6-rc-compile reads one directory containing service source definitions
        self.data_root.join("sv")
    }

    pub fn rc_root(&self) -> PathBuf {
        // Compiled databases live beside the stable compiled symlink
        self.data_root.join("rc")
    }

    pub fn compiled_link(&self) -> PathBuf {
        // s6-rc-init reads this stable link when a local live tree starts
        self.rc_root().join("compiled")
    }

    pub fn compile_command(&self, compiled: &Path) -> CommandSpec {
        let compiled = compiled.display().to_string();
        let source = self.source_root().display().to_string();
        // Direct compilation avoids assuming Artix's s6-db-reload helper exists
        CommandSpec::new(
            format!("s6-rc-compile {compiled} {source}"),
            "s6-rc-compile",
            [compiled, source],
        )
    }

    pub fn update_command(&self, compiled: &Path) -> CommandSpec {
        let live = self.live_root.display().to_string();
        let compiled = compiled.display().to_string();
        // The live tree is always passed explicitly because local s6 layouts differ
        CommandSpec::new(
            format!("s6-rc-update -l {live} {compiled}"),
            "s6-rc-update",
            ["-l".to_string(), live, compiled],
        )
    }
}
