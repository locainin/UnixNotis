//! Stat CSS token helpers

pub(super) fn stat_kind_css_class(kind: &str) -> Option<String> {
    super::super::kind_css::widget_kind_css_class("unixnotis-stat-kind-", kind)
}

#[cfg(test)]
#[path = "tests/style.rs"]
mod tests;
