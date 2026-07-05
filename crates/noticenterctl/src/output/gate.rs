pub(crate) fn allow_full_output(requested: bool, diagnostic_mode: bool) -> bool {
    requested && diagnostic_mode
}

pub(crate) fn warn_full_requires_diagnostic(requested: bool, diagnostic_mode: bool) -> bool {
    requested && !diagnostic_mode
}
