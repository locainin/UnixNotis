use std::fs;
use std::path::{Path, PathBuf};

use super::super::export_preset_from;
use super::support::TempDirGuard;
use crate::preset::archive::read_bundle;
use crate::preset::config_root::collect_selected_config_files_with_captures;

use super::super::script_dependencies::{
    collect_script_dependency_closure, normalize_relative_path, resolve_source_operand,
    SourceOperand, MAX_SCANNED_SCRIPT_BYTES,
};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

#[test]
fn export_includes_recursive_script_dir_source_dependencies() {
    let root = TempDirGuard::new("recursive-script-dependencies");
    root.write(
        "config.toml",
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"scripts/probe\"\n",
    );
    root.write("base.css", ".panel { color: red; }");
    root.write(
        "scripts/probe",
        "#!/bin/sh\nscript_dir=$(CDPATH='' cd -- \"$(dirname -- \"$0\")\" && pwd)\n. \"$script_dir/probe-lib\"\n",
    );
    root.write(
        "scripts/probe-lib",
        "#!/bin/sh\nsource \"${script_dir}/probe-values\"\n",
    );
    root.write("scripts/probe-values", "probe_value=42\n");
    root.write("scripts/unrelated", "private_value=1\n");

    let bundle_path = root.path.join("demo.unixnotis");
    let summary =
        export_preset_from(&root.path, &bundle_path, &[], false).expect("export dependencies");
    let bundle = read_bundle(&bundle_path).expect("read bundle");
    let paths = bundle
        .files
        .iter()
        .map(|file| file.relative_path.as_path())
        .collect::<Vec<_>>();

    assert_eq!(summary.file_count, 5);
    assert!(paths.contains(&Path::new("scripts/probe")));
    assert!(paths.contains(&Path::new("scripts/probe-lib")));
    assert!(paths.contains(&Path::new("scripts/probe-values")));
    assert!(!paths.contains(&Path::new("scripts/unrelated")));
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x5348_454c_4c50_4154),
        ..ProptestConfig::default()
    })]

    #[test]
    fn lexical_normalization_accepts_only_relative_contained_paths(
        depth in 1_usize..=12,
        backtracks in 0_usize..=12,
    ) {
        let mut candidate = PathBuf::new();
        for index in 0..depth {
            candidate.push(format!("part{index}"));
        }
        for _ in 0..backtracks {
            candidate.push("..");
        }
        candidate.push("helper.sh");

        let normalized = normalize_relative_path(&candidate);
        if backtracks <= depth {
            let normalized = normalized.expect("contained path should normalize");
            prop_assert!(!normalized.is_absolute());
            prop_assert!(normalized.components().all(|part| matches!(part, std::path::Component::Normal(_))));
        } else {
            prop_assert!(normalized.is_none());
        }
    }

    #[test]
    fn unsupported_source_operands_never_become_portable_paths(
        suffix in "[a-zA-Z0-9_./-]{0,256}",
    ) {
        let script = Path::new("scripts/status");
        for operand in [
            format!("${{dynamic}}/{suffix}"),
            format!("/etc/{suffix}"),
            format!("./{suffix}"),
        ] {
            prop_assert!(!matches!(
                resolve_source_operand(script, &operand),
                SourceOperand::Portable(_)
            ));
        }
    }
}

#[test]
fn export_rejects_a_missing_direct_script_dependency() {
    let root = TempDirGuard::new("missing-script-dependency");
    root.write(
        "config.toml",
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"scripts/probe\"\n",
    );
    root.write("base.css", ".panel { color: red; }");
    root.write(
        "scripts/probe",
        "#!/bin/sh\nscript_dir=$(dirname -- \"$0\")\n. \"$script_dir/missing-lib\"\n",
    );

    let bundle_path = root.path.join("demo.unixnotis");
    let error = export_preset_from(&root.path, &bundle_path, &[], false)
        .expect_err("missing dependency should stop export");

    assert!(error
        .to_string()
        .contains("script scripts/probe sources missing, unsafe, or oversized preset dependency scripts/missing-lib"));
    assert!(!bundle_path.exists());
}

#[test]
fn export_ignores_dynamic_and_absolute_source_operands() {
    let root = TempDirGuard::new("external-script-dependencies");
    root.write(
        "config.toml",
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"scripts/probe\"\n",
    );
    root.write("base.css", ".panel { color: red; }");
    root.write(
        "scripts/probe",
        "#!/bin/sh\n. \"$optional_library\"\n. \"`runtime-library`\"\n. /etc/os-release\nprintf '%s\\n' ok\n",
    );

    let bundle_path = root.path.join("demo.unixnotis");
    let summary =
        export_preset_from(&root.path, &bundle_path, &[], false).expect("export portable files");

    assert_eq!(summary.file_count, 3);
}

#[test]
fn exact_scan_limit_still_discovers_a_required_helper() {
    let root = TempDirGuard::new("script-scan-limit");
    // Keep the resource ceiling intentional so arithmetic mutations cannot silently weaken it
    assert_eq!(MAX_SCANNED_SCRIPT_BYTES, 1_048_576);
    root.write(
        "config.toml",
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"scripts/probe\"\n",
    );
    root.write("base.css", ".panel { color: red; }");
    let source_line = "#!/bin/sh\n. \"$script_dir/probe-lib\"\n";
    let padding_length = usize::try_from(MAX_SCANNED_SCRIPT_BYTES)
        .expect("scan limit should fit usize")
        .checked_sub(source_line.len())
        .expect("source line should fit scan limit");
    let mut script = String::with_capacity(
        usize::try_from(MAX_SCANNED_SCRIPT_BYTES).expect("scan limit should fit usize"),
    );
    script.push_str(source_line);
    script.extend(std::iter::repeat_n('#', padding_length));
    root.write("scripts/probe", &script);
    root.write("scripts/probe-lib", "probe_value=42\n");

    let bundle_path = root.path.join("demo.unixnotis");
    let summary =
        export_preset_from(&root.path, &bundle_path, &[], false).expect("export limit file");
    let bundle = read_bundle(&bundle_path).expect("read bundle");

    assert_eq!(summary.file_count, 4);
    assert!(bundle
        .files
        .iter()
        .any(|file| file.relative_path == Path::new("scripts/probe-lib")));
}

#[test]
fn oversized_entry_script_fails_closed_instead_of_omitting_helpers() {
    let root = TempDirGuard::new("oversized-script-entry");
    let script_size = usize::try_from(MAX_SCANNED_SCRIPT_BYTES + 1).expect("limit fits usize");
    root.write("scripts/probe", &"#".repeat(script_size));

    let error = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .err()
        .expect("oversized script must stop export");

    assert!(error
        .to_string()
        .contains("cannot verify dependencies for script scripts/probe"));
}

#[test]
fn safe_parent_relative_helper_is_normalized_inside_config_root() {
    let root = TempDirGuard::new("parent-relative-helper");
    root.write(
        "scripts/probe",
        "#!/bin/sh\n. \"$script_dir/../lib/common.sh\"\n",
    );
    root.write("lib/common.sh", "probe_value=42\n");

    let closure = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .expect("safe parent path should remain contained");

    assert_eq!(
        closure.paths,
        vec![
            PathBuf::from("lib/common.sh"),
            PathBuf::from("scripts/probe")
        ]
    );
}

#[test]
fn parent_relative_helpers_that_escape_config_root_stop_export() {
    let root = TempDirGuard::new("escaping-parent-helper");
    root.write(
        "scripts/probe",
        "#!/bin/sh\n. \"$script_dir/../../outside.sh\"\n",
    );

    let error = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .err()
        .expect("known config-relative escape must fail closed");

    assert!(error
        .to_string()
        .contains("escapes the UnixNotis config root"));
}

#[test]
fn ordinary_relative_source_operand_stops_export() {
    let root = TempDirGuard::new("working-directory-source");
    root.write("scripts/probe", "#!/bin/sh\n. ./lib/common.sh\n");

    let error = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .err()
        .expect("working-directory-relative source cannot be portable");

    assert!(error
        .to_string()
        .contains("depends on the runtime working directory"));
    assert!(error.to_string().contains("use $script_dir"));
}

#[test]
fn bare_source_operand_stops_export_instead_of_guessing_shell_path() {
    let root = TempDirGuard::new("shell-path-source");
    root.write("scripts/probe", "#!/bin/sh\n. common.sh\n");

    let error = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .err()
        .expect("bare source can use shell PATH and must not be guessed");

    assert!(error
        .to_string()
        .contains("depends on the runtime working directory"));
}

#[test]
fn non_utf8_shell_entry_stops_dependency_export() {
    let root = TempDirGuard::new("non-utf8-shell-entry");
    let script_path = root.path.join("scripts/probe");
    fs::create_dir_all(script_path.parent().expect("script parent")).expect("create scripts");
    fs::write(
        &script_path,
        b"#!/bin/sh\n# invalid byte: \xff\n. \"$script_dir/helper\"\n",
    )
    .expect("write non-UTF-8 shell script");

    let error = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .err()
        .expect("non-UTF-8 shell dependency cannot be verified");

    assert!(error
        .to_string()
        .contains("cannot verify non-UTF-8 shell dependency scripts/probe"));
}

#[test]
fn non_utf8_sourced_helper_stops_dependency_export_without_a_shebang() {
    let root = TempDirGuard::new("non-utf8-sourced-helper");
    root.write("scripts/probe", "#!/bin/sh\n. \"$script_dir/helper\"\n");
    fs::write(root.path.join("scripts/helper"), b"value='\xff'\n")
        .expect("write non-UTF-8 sourced helper");

    let error = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .err()
        .expect("sourced files are shell input even without a shebang");

    assert!(error
        .to_string()
        .contains("cannot verify non-UTF-8 shell dependency scripts/helper"));
}

#[test]
fn non_utf8_native_entry_remains_a_valid_bundled_executable() {
    let root = TempDirGuard::new("non-utf8-native-entry");
    let script_path = root.path.join("scripts/probe");
    fs::create_dir_all(script_path.parent().expect("script parent")).expect("create scripts");
    fs::write(&script_path, b"\x7fELF\x02\x01\xff").expect("write native executable bytes");

    let closure = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .expect("native executable does not require shell dependency scanning");

    assert_eq!(closure.paths, vec![PathBuf::from("scripts/probe")]);
}

#[test]
fn utf8_python_entry_with_source_assignment_is_bundled_without_shell_scanning() {
    let root = TempDirGuard::new("python-source-assignment");
    root.write(
        "config.toml",
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"scripts/probe.py\"\n",
    );
    root.write("base.css", ".panel { color: red; }");
    root.write(
        "scripts/probe.py",
        "#!/usr/bin/env python3\nsource = \"./not-a-shell-dependency\"\nprint(source)\n",
    );

    let bundle_path = root.path.join("python.unixnotis");
    export_preset_from(&root.path, &bundle_path, &[], false)
        .expect("Python entry should not use shell dependency parsing");
    let bundle = read_bundle(&bundle_path).expect("read Python preset");

    assert!(bundle
        .files
        .iter()
        .any(|file| file.relative_path == Path::new("scripts/probe.py")));
    assert!(!bundle
        .files
        .iter()
        .any(|file| file.relative_path == Path::new("not-a-shell-dependency")));
}

#[test]
fn env_shell_entry_with_option_value_still_discovers_source_dependencies() {
    let root = TempDirGuard::new("env-shell-options");
    root.write(
        "scripts/probe",
        "#!/usr/bin/env -S -u UNUSED sh\n. \"$script_dir/helper\"\n",
    );
    root.write("scripts/helper", "value=1\n");

    let closure = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .expect("env shell interpreter should remain scannable");

    assert_eq!(
        closure.paths,
        vec![
            PathBuf::from("scripts/helper"),
            PathBuf::from("scripts/probe")
        ]
    );
}

#[test]
fn env_shell_entry_skips_flags_and_assignments_before_interpreter() {
    let root = TempDirGuard::new("env-shell-flags");
    root.write(
        "scripts/probe",
        "#!/usr/bin/env -S -i MODE=test sh\n. \"$script_dir/helper\"\n",
    );
    root.write("scripts/helper", "value=1\n");

    let closure = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .expect("env flags and assignments should precede a shell interpreter");

    assert!(closure.paths.contains(&PathBuf::from("scripts/helper")));
}

#[test]
fn dependency_collection_reuses_the_bytes_scanned_securely() {
    let root = TempDirGuard::new("stable-script-capture");
    root.write("scripts/probe", "original=1\n");
    let closure = collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe")])
        .expect("capture script");

    root.write("scripts/probe", "replacement=1\n");
    let collected = collect_selected_config_files_with_captures(
        &root.path,
        &closure.paths,
        None,
        &[],
        &closure.captures,
    )
    .expect("collect captured script");

    assert_eq!(collected.files[0].source_contents, b"original=1\n");
}

#[test]
fn dependency_scan_rejects_non_regular_entry_paths() {
    let root = TempDirGuard::new("non-regular-entry");
    fs::create_dir_all(root.path.join("scripts/probe-dir")).expect("create command directory");

    let error =
        collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe-dir")])
            .err()
            .expect("directory entry should fail closed");

    assert!(error
        .to_string()
        .contains("cannot verify dependencies for script scripts/probe-dir"));
}
