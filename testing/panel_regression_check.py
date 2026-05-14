#!/usr/bin/env python3

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
PANEL_CSS = ROOT / "crates/unixnotis-core/assets/panel.css"
CONFIG_LAYOUT = ROOT / "crates/unixnotis-core/src/config/config_layout.rs"
EXEC_REVIEW = ROOT / "crates/noticenterctl/src/preset/import/exec_review.rs"


def require(condition: bool, label: str) -> None:
    if not condition:
        raise SystemExit(f"[fail] {label}")
    print(f"[pass] {label}")


def rule_body(css: str, selector: str) -> str:
    pattern = re.compile(rf"{re.escape(selector)}\s*\{{(?P<body>.*?)\n\}}", re.DOTALL)
    match = pattern.search(css)
    if not match:
        raise SystemExit(f"[fail] missing css rule: {selector}")
    return match.group("body")


def declaration_value(body: str, name: str) -> str:
    pattern = re.compile(rf"^\s*{re.escape(name)}\s*:\s*(?P<value>[^;]+);", re.MULTILINE)
    match = pattern.search(body)
    if not match:
        raise SystemExit(f"[fail] missing css declaration: {name}")
    return match.group("value").strip()


def check_stack_ghost_css() -> None:
    css = PANEL_CSS.read_text(encoding="utf-8")
    ghost = rule_body(css, ".unixnotis-stack-ghost")
    ghost_2 = rule_body(css, ".unixnotis-stack-ghost-2")
    collapsed = rule_body(css, ".unixnotis-panel-card-group-collapsed")

    require(
        declaration_value(collapsed, "margin-bottom") == "2px",
        "collapsed notification card does not reserve full row spacing",
    )
    require(
        declaration_value(ghost, "min-height") == "8px",
        "first stack ghost stays compact",
    )
    require(
        declaration_value(ghost, "margin-bottom") == "0",
        "first stack ghost removes inherited card bottom margin",
    )
    require(
        declaration_value(ghost_2, "min-height") == "7px",
        "second stack ghost stays compact",
    )
    require(
        declaration_value(ghost_2, "margin-bottom") == "0",
        "second stack ghost removes inherited card bottom margin",
    )


def check_default_panel_labels() -> None:
    source = CONFIG_LAYOUT.read_text(encoding="utf-8")
    require(
        "quick_actions_label: String::new()" in source,
        "default quick action heading stays opt-in",
    )
    require(
        "system_status_label: String::new()" in source,
        "default system status heading stays opt-in",
    )
    require(
        '"Quick Actions".to_string()' not in source,
        "quick action heading is not hardcoded by default",
    )
    require(
        '"System Status".to_string()' not in source,
        "system status heading is not hardcoded by default",
    )


def check_exec_review_test_is_terminal_stable() -> None:
    source = EXEC_REVIEW.read_text(encoding="utf-8")
    require(
        "render_exec_content_review_with_style" in source,
        "exec review tests can bypass terminal color detection",
    )
    require(
        "ReviewStyle { color: false }" in source,
        "exec review command assertion uses plain deterministic output",
    )


def main() -> int:
    check_stack_ghost_css()
    check_default_panel_labels()
    check_exec_review_test_is_terminal_stable()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
