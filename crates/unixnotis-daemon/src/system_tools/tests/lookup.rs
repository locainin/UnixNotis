use super::super::lookup::trusted_program_path;

#[test]
fn production_lookup_rejects_empty_and_path_shaped_programs_before_scanning() {
    assert!(trusted_program_path("").is_none());
    assert!(trusted_program_path("relative/tool").is_none());
    assert!(trusted_program_path("/absolute/tool").is_none());
}
