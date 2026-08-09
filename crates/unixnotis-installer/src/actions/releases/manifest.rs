//! Release manifest construction and installed generation verification

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unixnotis_core::filesystem::{open_regular_file, read_regular_file_bounded};

use crate::managed_binaries::is_managed_binary_name;
use crate::paths::InstallPaths;

pub(super) const INSTALLED_MANIFEST_FILE: &str = "manifest.json";
const INSTALLED_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub(in crate::actions::releases) const MAX_INSTALLED_MANIFEST_BYTES: u64 = 256 * 1024;
pub(in crate::actions::releases) const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct InstalledReleaseManifest {
    pub(super) schema_version: u32,
    pub(super) package_version: String,
    pub(super) build_id: String,
    pub(super) binaries: BTreeMap<String, BinaryManifest>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct BinaryManifest {
    pub(super) size: u64,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::actions) enum BinaryHealth {
    Missing,
    Healthy {
        generation: String,
        package_version: String,
        digest: String,
    },
    WrongType,
    NotExecutable,
    BrokenLink,
    WrongGeneration,
    HashMismatch,
    Unsafe(String),
}

impl BinaryHealth {
    pub(in crate::actions) const fn label(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Healthy { .. } => "healthy",
            Self::WrongType => "wrong type",
            Self::NotExecutable => "not executable",
            Self::BrokenLink => "broken link",
            Self::WrongGeneration => "wrong generation",
            Self::HashMismatch => "hash mismatch",
            Self::Unsafe(_) => "unsafe",
        }
    }
}

pub(super) fn build_manifest(sources: &[(String, PathBuf)]) -> Result<InstalledReleaseManifest> {
    let mut binaries = BTreeMap::new();
    for (name, source) in sources {
        // One no-follow descriptor ties size, mode, and digest to the same source object
        let mut file =
            open_regular_file(source).with_context(|| format!("open build artifact {name}"))?;
        let metadata = file
            .metadata()
            .with_context(|| format!("inspect build artifact {name}"))?;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(anyhow!("build artifact is not executable: {name}"));
        }
        binaries.insert(
            name.clone(),
            BinaryManifest {
                size: metadata.len(),
                sha256: sha256_open_file(&mut file, source)
                    .with_context(|| format!("hash build artifact {name}"))?,
            },
        );
    }
    let package_version = env!("CARGO_PKG_VERSION").to_string();
    let build_id = release_build_id(&package_version, &binaries);
    Ok(InstalledReleaseManifest {
        schema_version: INSTALLED_MANIFEST_SCHEMA_VERSION,
        package_version,
        build_id,
        binaries,
    })
}

pub(super) fn verify_release_directory(
    release_dir: &Path,
    expected: &InstalledReleaseManifest,
) -> Result<()> {
    let stored = read_manifest(&release_dir.join(INSTALLED_MANIFEST_FILE))?;
    if &stored != expected {
        return Err(anyhow!(
            "installed release manifest does not match staged generation"
        ));
    }
    for (name, binary) in &stored.binaries {
        let path = release_dir.join("bin").join(name);
        let mut file = open_regular_file(&path)
            .with_context(|| format!("open installed release binary {name}"))?;
        let metadata = file
            .metadata()
            .with_context(|| format!("inspect installed release binary {name}"))?;
        if metadata.len() != binary.size {
            return Err(anyhow!(
                "installed release binary shape or size mismatch: {name}"
            ));
        }
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(anyhow!(
                "installed release binary is not executable: {name}"
            ));
        }
        if sha256_open_file(&mut file, &path)? != binary.sha256 {
            return Err(anyhow!("installed release binary digest mismatch: {name}"));
        }
    }
    Ok(())
}

pub(in crate::actions) fn inspect_installed_generation(
    paths: &InstallPaths,
    binaries: &[String],
) -> Vec<(String, BinaryHealth)> {
    let current = match paths.installed_current_link() {
        Ok(current) => current,
        Err(error) => {
            return binaries
                .iter()
                .cloned()
                .map(|name| (name, BinaryHealth::Unsafe(error.to_string())))
                .collect()
        }
    };
    let current_target = match std::fs::read_link(&current) {
        Ok(target) => target,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return binaries
                .iter()
                .cloned()
                .map(|name| {
                    let entry = paths.bin_dir.join(&name);
                    let health = classify_missing_generation_entry(&entry);
                    (name, health)
                })
                .collect()
        }
        Err(error) => {
            return binaries
                .iter()
                .cloned()
                .map(|name| (name, BinaryHealth::Unsafe(error.to_string())))
                .collect()
        }
    };
    if !is_release_target(&current_target) {
        return binaries
            .iter()
            .cloned()
            .map(|name| (name, BinaryHealth::WrongGeneration))
            .collect();
    }
    let release_dir = paths
        .installed_release_root()
        .map(|root| root.join(&current_target));
    let Ok(release_dir) = release_dir else {
        return binaries
            .iter()
            .cloned()
            .map(|name| (name, BinaryHealth::WrongGeneration))
            .collect();
    };
    let manifest = match read_manifest(&release_dir.join(INSTALLED_MANIFEST_FILE)) {
        Ok(manifest) => manifest,
        Err(error) => {
            return binaries
                .iter()
                .cloned()
                .map(|name| (name, BinaryHealth::Unsafe(error.to_string())))
                .collect()
        }
    };
    let expected_generation = format!("{}-{}", manifest.package_version, manifest.build_id);
    // The current link names the same generation proven by the content manifest digest
    if current_target.file_name().and_then(|name| name.to_str())
        != Some(expected_generation.as_str())
    {
        return binaries
            .iter()
            .cloned()
            .map(|name| (name, BinaryHealth::WrongGeneration))
            .collect();
    }
    let generation = manifest.build_id.clone();
    let entry_target = entrypoint_target();

    binaries
        .iter()
        .map(|name| {
            let entry = paths.bin_dir.join(name);
            let health = inspect_binary_entry(
                &entry,
                &entry_target.join(name),
                &release_dir,
                &manifest,
                name,
                &generation,
            );
            (name.clone(), health)
        })
        .collect()
}

fn is_release_target(target: &Path) -> bool {
    let mut components = target.components();
    matches!(
        (components.next(), components.next(), components.next()),
        (
            Some(std::path::Component::Normal(root)),
            Some(std::path::Component::Normal(_generation)),
            None
        ) if root == "releases"
    )
}

fn classify_missing_generation_entry(entry: &Path) -> BinaryHealth {
    match std::fs::symlink_metadata(entry) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BinaryHealth::Missing,
        Ok(metadata) if metadata.file_type().is_symlink() => BinaryHealth::BrokenLink,
        Ok(_metadata) => BinaryHealth::WrongGeneration,
        Err(error) => BinaryHealth::Unsafe(error.to_string()),
    }
}

fn inspect_binary_entry(
    entry: &Path,
    expected_link: &Path,
    release_dir: &Path,
    manifest: &InstalledReleaseManifest,
    name: &str,
    generation: &str,
) -> BinaryHealth {
    let metadata = match std::fs::symlink_metadata(entry) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return BinaryHealth::Missing,
        Err(error) => return BinaryHealth::Unsafe(error.to_string()),
    };
    if !metadata.file_type().is_symlink() {
        return BinaryHealth::WrongType;
    }
    match std::fs::read_link(entry) {
        Ok(target) if target == expected_link => {}
        Ok(_target) => return BinaryHealth::WrongGeneration,
        Err(error) => return BinaryHealth::Unsafe(error.to_string()),
    }
    let Some(expected) = manifest.binaries.get(name) else {
        return BinaryHealth::WrongGeneration;
    };
    let binary = release_dir.join("bin").join(name);
    // The retained descriptor keeps health metadata and hashing on one exact object
    let mut file = match open_regular_file(&binary) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return BinaryHealth::BrokenLink
        }
        Err(error) => return BinaryHealth::Unsafe(error.to_string()),
    };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => return BinaryHealth::Unsafe(error.to_string()),
    };
    if metadata.len() != expected.size {
        return BinaryHealth::WrongType;
    }
    if metadata.permissions().mode() & 0o111 == 0 {
        return BinaryHealth::NotExecutable;
    }
    match sha256_open_file(&mut file, &binary) {
        Ok(digest) if digest == expected.sha256 => BinaryHealth::Healthy {
            generation: generation.to_string(),
            package_version: manifest.package_version.clone(),
            digest,
        },
        Ok(_digest) => BinaryHealth::HashMismatch,
        Err(error) => BinaryHealth::Unsafe(error.to_string()),
    }
}

pub(super) fn read_manifest(path: &Path) -> Result<InstalledReleaseManifest> {
    let bytes = read_regular_file_bounded(path, MAX_INSTALLED_MANIFEST_BYTES)
        .with_context(|| format!("read installed release manifest {}", path.display()))?;
    let manifest: InstalledReleaseManifest =
        serde_json::from_slice(&bytes).with_context(|| "parse installed release manifest")?;
    if manifest.schema_version != INSTALLED_MANIFEST_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported installed release manifest schema {}",
            manifest.schema_version
        ));
    }
    if manifest.binaries.is_empty()
        || manifest
            .binaries
            .keys()
            .any(|name| !is_managed_binary_name(name))
    {
        return Err(anyhow!(
            "installed release manifest contains unmanaged binary names"
        ));
    }
    let expected_build_id = release_build_id(&manifest.package_version, &manifest.binaries);
    if manifest.build_id != expected_build_id {
        return Err(anyhow!(
            "installed release manifest build identity is inconsistent"
        ));
    }
    Ok(manifest)
}

pub(super) fn manifest_bytes(manifest: &InstalledReleaseManifest) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(manifest).with_context(|| "serialize installed release manifest")
}

pub(in crate::actions) fn entrypoint_target() -> PathBuf {
    PathBuf::from("..")
        .join("lib")
        .join("unixnotis")
        .join("current")
        .join("bin")
}

fn release_build_id(package_version: &str, binaries: &BTreeMap<String, BinaryManifest>) -> String {
    let mut digest = Sha256::new();
    digest.update(package_version.as_bytes());
    for (name, binary) in binaries {
        digest.update(name.as_bytes());
        digest.update(binary.size.to_le_bytes());
        digest.update(binary.sha256.as_bytes());
    }
    format_digest(&digest.finalize())
}

fn sha256_open_file(file: &mut File, path: &Path) -> Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = vec![0u8; HASH_BUFFER_BYTES].into_boxed_slice();
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format_digest(&digest.finalize()))
}

fn format_digest(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
