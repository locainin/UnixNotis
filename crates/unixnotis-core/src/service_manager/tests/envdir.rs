use super::super::envdir_file_contents;

#[test]
fn envdir_contents_keep_only_the_trimmed_first_line() {
    assert_eq!(
        envdir_file_contents(Some("wayland-1  \nignored")),
        "wayland-1\n"
    );
    assert_eq!(envdir_file_contents(Some("value\0ignored")), "value\n");
}

#[test]
fn missing_envdir_value_creates_an_empty_unset_marker() {
    assert_eq!(envdir_file_contents(None), "");
}
