//! Narrow source directives for intentional CSS cascade overrides

use std::ops::Range;

const ALLOW_DUPLICATE_SELECTORS_START: &str =
    "/* unixnotis-css-check allow-duplicate-selectors:start */";
const ALLOW_DUPLICATE_SELECTORS_END: &str =
    "/* unixnotis-css-check allow-duplicate-selectors:end */";

#[derive(Debug, Default)]
pub(super) struct DuplicateSelectorAllowlist {
    ranges: Vec<Range<usize>>,
}

impl DuplicateSelectorAllowlist {
    pub(super) fn from_source(source: &str) -> Self {
        let mut ranges = Vec::new();
        let mut remaining = source;

        while let Some((_before_start, after_start)) =
            remaining.split_once(ALLOW_DUPLICATE_SELECTORS_START)
        {
            let Some((allowed_source, after_end)) =
                after_start.split_once(ALLOW_DUPLICATE_SELECTORS_END)
            else {
                // An incomplete directive must not hide the rest of a user stylesheet
                break;
            };
            // Slice lengths provide absolute offsets without letting malformed input overflow
            let start = source
                .len()
                .checked_sub(after_start.len())
                .expect("directive slice belongs to source");
            let end = start
                .checked_add(allowed_source.len())
                .expect("allowed directive range fits source");
            ranges.push(start..end);
            // Splitting consumes one complete section and guarantees forward progress
            remaining = after_end;
        }

        if ranges.is_empty() {
            // Existing untouched installs predate directives but retain known stock bytes
            if let Some(start) = legacy_stock_override_start(source) {
                ranges.push(start..source.len());
            }
        }

        Self { ranges }
    }

    pub(super) fn contains(&self, offset: usize) -> bool {
        self.ranges.iter().any(|range| range.contains(&offset))
    }
}

fn legacy_stock_override_start(source: &str) -> Option<usize> {
    [
        (
            unixnotis_core::DEFAULT_PANEL_CSS,
            "/* Restrained default composition",
        ),
        (
            unixnotis_core::DEFAULT_WIDGETS_CSS,
            "/* Restrained widget composition",
        ),
        (
            unixnotis_core::DEFAULT_MEDIA_CSS,
            "/* Restrained media transport */",
        ),
    ]
    .into_iter()
    .find_map(|(current, override_header)| {
        let legacy = current
            .replace(&format!("{ALLOW_DUPLICATE_SELECTORS_START}\n"), "")
            .replace(&format!("{ALLOW_DUPLICATE_SELECTORS_END}\n"), "");
        (source == legacy).then(|| {
            source
                .find(override_header)
                .expect("stock override header remains present")
        })
    })
}

#[cfg(test)]
#[path = "tests/directives.rs"]
mod tests;
