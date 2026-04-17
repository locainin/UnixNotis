pub(super) fn max_optional_heights<const N: usize>(heights: [Option<i32>; N]) -> Option<i32> {
    // Optional rows appear and disappear by shell mode, so the tallest visible one wins
    heights.into_iter().flatten().max()
}

pub(super) fn append_vertical(
    first: impl Into<Option<i32>>,
    second: impl Into<Option<i32>>,
    spacing_px: i32,
) -> Option<i32> {
    // Vertical append is used for shell bands that stack one after another
    match (first.into(), second.into()) {
        (Some(first), Some(second)) => Some(first + spacing_px + second),
        (Some(first), None) => Some(first),
        (None, Some(second)) => Some(second),
        (None, None) => None,
    }
}

pub(super) fn stack_visible_heights(heights: &[Option<i32>], spacing_px: i32) -> i32 {
    // Text rows share one spacing rule and ignore rows that are hidden
    let mut total_px = 0;
    for (visible, height) in heights.iter().flatten().enumerate() {
        if visible > 0 {
            total_px += spacing_px;
        }
        total_px += *height;
    }
    total_px
}
