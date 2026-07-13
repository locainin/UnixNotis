pub const fn allow_full_output(requested: bool, diagnostic_mode: bool) -> bool {
    requested && diagnostic_mode
}

pub const fn warn_full_requires_diagnostic(requested: bool, diagnostic_mode: bool) -> bool {
    requested && !diagnostic_mode
}
