use std::collections::HashSet;
use std::path::PathBuf;

use unixnotis_core::{AttributionClass, InlineReplyPolicy};

use super::*;
use crate::daemon::notifications::identity::desktop_index::model::{
    ExecutableIdentity, LaunchArgument, LaunchSpec, LiteralArgument,
};
use crate::daemon::notifications::identity::desktop_index::{DesktopIdentityIndex, DesktopRecord};
use crate::daemon::notifications::identity::FileIdentity;

trait DesktopRecordFixture {
    fn fixture(
        id: &str,
        display_name: &str,
        executable_path: &str,
        identity: FileIdentity,
        system_entry: bool,
        dbus_activatable: bool,
    ) -> Self;

    fn with_launch_literals(self, arguments: &[&str]) -> Self;
}

impl DesktopRecordFixture for DesktopRecord {
    fn fixture(
        id: &str,
        display_name: &str,
        executable_path: &str,
        identity: FileIdentity,
        system_entry: bool,
        dbus_activatable: bool,
    ) -> Self {
        Self {
            id: id.to_string(),
            display_name: display_name.to_string(),
            badge_icon: id.to_string(),
            executable_path: Some(PathBuf::from(executable_path)),
            executable_identity: Some(identity),
            desktop_identity: Some(identity),
            system_origin: system_entry,
            system_association: system_entry,
            association_eligible: true,
            dbus_activatable,
            launch_spec: Some(LaunchSpec {
                executable: identity,
                arguments: Vec::new(),
                literal_files_are_system_managed: true,
            }),
            names: HashSet::from([normalize_name(display_name)]),
        }
    }

    fn with_launch_literals(mut self, arguments: &[&str]) -> Self {
        let executable = self
            .executable_identity
            .expect("launch fixture needs executable identity");
        self.launch_spec = Some(LaunchSpec {
            executable,
            arguments: arguments
                .iter()
                .map(|value| {
                    LaunchArgument::Literal(LiteralArgument {
                        value: value.as_bytes().to_vec(),
                        file: None,
                    })
                })
                .collect(),
            literal_files_are_system_managed: true,
        });
        self
    }
}

trait DesktopIdentityIndexFixture {
    fn from_records(
        records: Vec<DesktopRecord>,
        trusted_relays: Vec<(PathBuf, FileIdentity)>,
    ) -> Self;

    fn with_trusted_portal(self, path: PathBuf, identity: FileIdentity) -> Self;
}

impl DesktopIdentityIndexFixture for DesktopIdentityIndex {
    fn from_records(
        records: Vec<DesktopRecord>,
        trusted_relays: Vec<(PathBuf, FileIdentity)>,
    ) -> Self {
        let mut index = Self::default();
        for record in records {
            index.index_record(record);
        }
        index.trusted_relays = trusted_relays
            .into_iter()
            .map(|(path, identity)| ExecutableIdentity { path, identity })
            .collect();
        index
    }

    fn with_trusted_portal(mut self, path: PathBuf, identity: FileIdentity) -> Self {
        index_trusted_portal(&mut self, path, identity);
        self
    }
}

fn index_trusted_portal(index: &mut DesktopIdentityIndex, path: PathBuf, identity: FileIdentity) {
    index
        .trusted_portals
        .push(ExecutableIdentity { path, identity });
}

fn identity(device: u64, inode: u64, uid: u32) -> FileIdentity {
    FileIdentity {
        device,
        inode,
        uid,
        mode: 0o100_755,
    }
}

fn sender(path: &str, identity: FileIdentity) -> SenderMetadata {
    SenderMetadata {
        sender_name: Some(":1.42".to_string()),
        sender_executable: Some(path.to_string()),
        sender_executable_identity: Some(identity),
        sender_cmdline: Some(vec![path.as_bytes().to_vec()]),
        ..SenderMetadata::default()
    }
}

fn sender_with_arguments(path: &str, identity: FileIdentity, arguments: &[&str]) -> SenderMetadata {
    let mut metadata = sender(path, identity);
    metadata.sender_cmdline = Some(
        std::iter::once(path)
            .chain(arguments.iter().copied())
            .map(|argument| argument.as_bytes().to_vec())
            .collect(),
    );
    metadata
}

fn system_record(id: &str, name: &str, path: &str, identity: FileIdentity) -> DesktopRecord {
    DesktopRecord::fixture(id, name, path, identity, true, false)
}

fn installed_system_executable() -> (String, FileIdentity) {
    let path = unixnotis_core::util::trusted_system_program_path("true")
        .expect("find a protected system executable");
    let evidence = executable_evidence_for_path(&path).expect("read system executable evidence");
    assert!(evidence.identity.is_system_managed());
    assert!(evidence.identity.is_executable_regular());
    (path.display().to_string(), evidence.identity)
}

#[path = "resolver/association.rs"]
mod association;
#[path = "resolver/portal.rs"]
mod portal;
#[path = "resolver/runtime.rs"]
mod runtime;
#[path = "resolver/spoof.rs"]
mod spoof;
