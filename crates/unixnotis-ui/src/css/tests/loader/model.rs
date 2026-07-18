use super::*;

#[test]
fn load_result_constructors_keep_source_and_error_consistent() {
    assert_eq!(
        CssFileLoadResult::custom().source,
        CssFileLoadSource::Custom
    );
    assert_eq!(
        CssFileLoadResult::empty_fallback().source,
        CssFileLoadSource::EmptyFallback
    );
    let failed = CssFileLoadResult::read_failure("permission denied".to_string());
    assert_eq!(failed.source, CssFileLoadSource::ReadFailureFallback);
    assert_eq!(failed.error.as_deref(), Some("permission denied"));
}
