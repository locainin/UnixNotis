//! Shared envdir value encoding for runit and s6 service environments

/// Convert one optional environment value to chpst/s6-envdir file contents
#[must_use]
pub fn envdir_file_contents(value: Option<&str>) -> String {
    value.map_or_else(String::new, |value| {
        let first_line = value
            .split(['\0', '\n'])
            .next()
            .unwrap_or_default()
            .trim_end_matches([' ', '\t']);
        format!("{first_line}\n")
    })
}

#[cfg(test)]
#[path = "tests/envdir.rs"]
mod tests;
