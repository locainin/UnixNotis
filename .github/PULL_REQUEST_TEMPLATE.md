## Summary

Describe the change in plain terms

- what changed:
- why:

## Problem

What user-visible bug, regression, limitation, or maintenance issue does this fix

## Linked Issue

Link the issue if there is one

- fixes:
- related:

## Changes

List the actual work done here

- code changes:
- behavior changes:
- tests or diagnostics added:

## Root Cause

For bug fixes, explain what was actually wrong

- trigger:
- root cause:
- why this fix is the right one:

## Testing

Run the full workspace checks and list any extra verification that was done

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Extra verification:

- manual checks:
- screenshots or terminal output:

## Config / Docs

Note anything that needs to be called out for users or maintainers

- [ ] `config.toml` behavior changed
- [ ] CSS or theme behavior changed
- [ ] CLI behavior or flags changed
- [ ] D-Bus behavior changed
- [ ] Wiki docs need an update
- [ ] `README.md` or `CONTRIBUTING.md` need an update

If docs were not updated in this PR, note what should be updated later

## Review Notes

Call out anything reviewers should pay extra attention to

- main files to review:
- edge cases checked:
- tradeoffs or limitations:

## Checklist

- [ ] The change stays focused on one problem or one closely related set of problems
- [ ] The problem and root cause are explained clearly
- [ ] Full workspace tests passed
- [ ] Full workspace clippy passed
- [ ] New behavior was checked for regressions
- [ ] Logs, errors, and warnings are still clear
- [ ] No unnecessary background work, polling, or unbounded state was added
- [ ] Any new config, CSS, or docs impact is noted above
