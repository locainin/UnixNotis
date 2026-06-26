use super::{extract_bins_from_metadata, parse_install_binaries_metadata, CargoMetadata};

#[test]
fn parse_install_binaries_metadata_reads_entries() {
    // Metadata order is preserved because installer logs and plans should stay predictable
    let input = r#"
[workspace.metadata.unixnotis.installer]
binaries = ["unixnotis-daemon", "noticenterctl", "unixnotis-daemon"]
"#;
    let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
    assert_eq!(
        binaries,
        vec!["unixnotis-daemon".to_string(), "noticenterctl".to_string()]
    );
}

#[test]
fn parse_install_binaries_metadata_handles_missing_table() {
    // Missing installer metadata means discovery can fall back to cargo metadata
    let input = r#"
[workspace]
members = ["crates/unixnotis-daemon"]
"#;
    let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
    assert!(binaries.is_empty());
}

#[test]
fn parse_install_binaries_metadata_handles_empty_entries() {
    // Blank entries are ignored rather than becoming invalid binary names
    let input = r#"
[workspace.metadata.unixnotis.installer]
binaries = ["unixnotis-daemon", "  ", ""]
"#;
    let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
    assert_eq!(binaries, vec!["unixnotis-daemon".to_string()]);
}

#[test]
fn parse_install_binaries_metadata_keeps_first_duplicate_only() {
    // De-duplication keeps the first declaration so repeated names do not alter install order
    let input = r#"
[workspace.metadata.unixnotis.installer]
binaries = ["unixnotis-popups", "unixnotis-daemon", "unixnotis-popups"]
"#;
    let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
    assert_eq!(
        binaries,
        vec![
            "unixnotis-popups".to_string(),
            "unixnotis-daemon".to_string()
        ]
    );
}

#[test]
fn extract_bins_from_cargo_metadata_skips_installer_binary() {
    let input = r#"
{
  "target_directory": "target",
  "packages": [
    {
      "targets": [
        { "name": "unixnotis-daemon", "kind": ["bin"] },
        { "name": "unixnotis-installer", "kind": ["bin"] },
        { "name": "unixnotis-core", "kind": ["lib"] }
      ]
    }
  ]
}
"#;
    let metadata: CargoMetadata = serde_json::from_str(input).expect("metadata");
    let binaries = extract_bins_from_metadata(&metadata);
    assert_eq!(binaries, vec!["unixnotis-daemon".to_string()]);
}
