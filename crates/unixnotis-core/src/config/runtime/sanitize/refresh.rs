pub(super) fn clamp_refresh_interval(value: u64, min: u64, max: u64) -> u64 {
    // Zero keeps the interval disabled instead of forcing polling back on
    if value == 0 {
        return 0;
    }
    value.clamp(min, max)
}
