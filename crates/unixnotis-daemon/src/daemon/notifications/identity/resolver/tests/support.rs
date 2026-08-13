//! Shared resolver fixtures and synthetic process-evidence builders

use super::*;

pub(super) trait DesktopRecordFixture {
    fn fixture(
        id: &str,
        display_name: &str,
        executable_path: &str,
        identity: FileIdentity,
        system_entry: bool,
    ) -> Self;

    fn with_launch_literals(self, arguments: &[&str]) -> Self;

    fn with_protected_launch_file(self, path: &str, identity: FileIdentity) -> Self;
}

impl DesktopRecordFixture for DesktopRecord {
    fn fixture(
        id: &str,
        display_name: &str,
        executable_path: &str,
        identity: FileIdentity,
        system_entry: bool,
    ) -> Self {
        Self {
            id: id.to_string(),
            display_name: display_name.to_string(),
            badge_icon: id.to_string(),
            desktop_path: Some(PathBuf::from(format!(
                "/usr/share/applications/{id}.desktop"
            ))),
            declared_executable_path: Some(PathBuf::from(executable_path)),
            declared_executable_identity: Some(identity),
            runtime_executable_path: Some(PathBuf::from(executable_path)),
            runtime_executable_identity: Some(identity),
            desktop_identity: Some(identity),
            desktop_provenance: if system_entry {
                InstallProvenance::Package {
                    provider: PackageProvider::Pacman,
                    package_id: id.to_string(),
                }
            } else {
                InstallProvenance::Unknown
            },
            declared_executable_provenance: if system_entry {
                InstallProvenance::Package {
                    provider: PackageProvider::Pacman,
                    package_id: id.to_string(),
                }
            } else {
                InstallProvenance::Unknown
            },
            runtime_executable_provenance: if system_entry {
                InstallProvenance::Package {
                    provider: PackageProvider::Pacman,
                    package_id: id.to_string(),
                }
            } else {
                InstallProvenance::Unknown
            },
            system_origin: system_entry,
            system_association: system_entry,
            association_eligible: true,
            launch_spec: Some(LaunchSpec {
                declared_executable: identity,
                runtime_executable: identity,
                arguments: Vec::new(),
                environment: Vec::new(),
                wrappers: Vec::new(),
                package_launcher: None,
                literal_files_are_system_managed: true,
            }),
            names: HashSet::from([normalize_name(display_name)]),
        }
    }

    fn with_launch_literals(mut self, arguments: &[&str]) -> Self {
        let executable = self
            .runtime_executable_identity
            .expect("launch fixture needs executable identity");
        self.launch_spec = Some(LaunchSpec {
            declared_executable: executable,
            runtime_executable: executable,
            arguments: arguments
                .iter()
                .map(|value| {
                    LaunchArgument::Literal(LiteralArgument {
                        value: value.as_bytes().to_vec(),
                        file: None,
                    })
                })
                .collect(),
            environment: Vec::new(),
            wrappers: Vec::new(),
            package_launcher: None,
            literal_files_are_system_managed: true,
        });
        self
    }

    fn with_protected_launch_file(mut self, path: &str, identity: FileIdentity) -> Self {
        let spec = self
            .launch_spec
            .as_mut()
            .expect("launch fixture needs a launch specification");
        let literal = spec
            .arguments
            .iter_mut()
            .find_map(|argument| match argument {
                LaunchArgument::Literal(literal) if literal.value == path.as_bytes() => {
                    Some(literal)
                }
                _ => None,
            })
            .expect("protected launch path must exist in the fixture contract");
        literal.file = Some((PathBuf::from(path), identity));
        self
    }
}

pub(super) trait DesktopIdentityIndexFixture {
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

pub(super) fn identity(device: u64, inode: u64, uid: u32) -> FileIdentity {
    FileIdentity {
        device,
        inode,
        uid,
        mode: 0o100_755,
    }
}

pub(super) fn package(package_id: &str) -> InstallProvenance {
    InstallProvenance::Package {
        provider: PackageProvider::Pacman,
        package_id: package_id.to_string(),
    }
}

pub(super) fn sender(path: &str, identity: FileIdentity) -> SenderMetadata {
    SenderMetadata {
        sender_name: Some(":1.42".to_string()),
        sender_executable: Some(path.to_string()),
        sender_executable_identity: Some(identity),
        command_line: CommandLineEvidence {
            argv: vec![path.as_bytes().to_vec()],
            quality: CommandLineQuality::Structured,
        },
        ..SenderMetadata::default()
    }
}

pub(super) fn sender_with_arguments(
    path: &str,
    identity: FileIdentity,
    arguments: &[&str],
) -> SenderMetadata {
    let mut metadata = sender(path, identity);
    metadata.command_line = CommandLineEvidence {
        argv: std::iter::once(path)
            .chain(arguments.iter().copied())
            .map(|argument| argument.as_bytes().to_vec())
            .collect(),
        quality: CommandLineQuality::Structured,
    };
    metadata
}

pub(super) fn system_record(
    id: &str,
    name: &str,
    path: &str,
    identity: FileIdentity,
) -> DesktopRecord {
    DesktopRecord::fixture(id, name, path, identity, true)
}

pub(super) fn installed_system_executable() -> (String, FileIdentity) {
    let path = unixnotis_core::util::trusted_system_program_path("true")
        .expect("find a protected system executable");
    let evidence = executable_evidence_for_path(&path).expect("read system executable evidence");
    assert!(
        evidence.identity.is_system_managed(),
        "fixture executable should be system managed"
    );
    assert!(
        evidence.identity.is_executable_regular(),
        "fixture executable should be a regular executable"
    );
    (path.display().to_string(), evidence.identity)
}

pub(super) fn verified_executable_record<'record>(
    records: &[&'record DesktopRecord],
    reported_name: &str,
    sender: &SenderMetadata,
    index: &DesktopIdentityIndex,
) -> Option<VerifiedDesktopRecord<'record>> {
    let results = records
        .iter()
        .map(|record| CandidateVerification {
            record,
            verification: verify_record_sender(record, sender, index),
        })
        .collect::<Vec<_>>();
    strongest_verified_result(&results, reported_name, index)
}
