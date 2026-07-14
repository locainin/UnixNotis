use std::fs;
use std::path::{Path, PathBuf};

use super::export_preset_from;
use crate::preset::archive::read_bundle;

use super::script_dependencies::{collect_script_dependency_closure, MAX_SCANNED_SCRIPT_BYTES};
use super::tests::TempDirGuard;

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
        .contains("script scripts/probe sources missing preset dependency scripts/missing-lib"));
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
    let source_line = ". \"$script_dir/probe-lib\"\n";
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
fn dependency_scan_skips_non_regular_entry_paths() {
    let root = TempDirGuard::new("non-regular-entry");
    fs::create_dir_all(root.path.join("scripts/probe-dir")).expect("create command directory");

    let closure =
        collect_script_dependency_closure(&root.path, &[PathBuf::from("scripts/probe-dir")])
            .expect("directory entry should be skipped safely");

    assert_eq!(closure, vec![PathBuf::from("scripts/probe-dir")]);
}
