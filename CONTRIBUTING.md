# Contributing to UnixNotis

Thanks for taking the time to contribute.

UnixNotis is a Wayland-first notification system with a D-Bus daemon, a control-center panel, toast popups, and supporting CLI and installer tooling. Contributions should improve the project without making it heavier, less predictable, or harder to maintain.

## Before contributing

- Read the [README](README.md) first
- Check the GitHub Wiki for configuration and styling details
- Keep changes focused
- Prefer fixing one problem well over mixing unrelated cleanup into the same pull request

## What we are looking for

Good contributions usually do one or more of these:

- Fix a bug or regression
- Improve reliability during long-running sessions
- Reduce unnecessary CPU, memory, or I/O work
- Improve maintainability without changing behavior
- Improve documentation, diagnostics, or test coverage
- Keep behavior aligned with the project's Wayland-first scope

## Scope and design expectations

Please keep contributions in scope for the project.

- Keep UnixNotis focused on being a notification system and related tooling
- Avoid feature creep that turns it into a general desktop shell or widget framework
- Do not add large new dependencies without a strong reason
- Do not add background work, polling, or timers unless they are clearly necessary and bounded
- Prefer event-driven behavior over periodic work when possible
- Keep memory use predictable and avoid unbounded queues, caches, or retries

## Performance expectations

Performance matters here.

- New code should avoid waste when idle
- Repeated work should be coalesced, cached, or bounded where appropriate
- Long-running processes should not slowly accumulate memory or CPU overhead
- Shell commands, subprocesses, watchers, and async tasks should have a clear lifecycle

Good rules of thumb:

- Avoid busy loops
- Avoid waking the UI just to discover nothing changed
- Avoid doing the same parse, decode, or command twice when one result would do
- Avoid silent fallback behavior that hides broken or expensive paths

## Code style

- Keep files organized and responsibilities clear
- Prefer small focused modules over one oversized file
- Keep code readable and boring in the good way
- Add comments where they actually help future maintenance
- Avoid drive-by refactors unless they are necessary for the change
- Keep naming direct and consistent with the existing codebase

## Testing

At a minimum, all code changes should pass the following:

```sh
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery -W clippy::restriction
```
If a full run is not practical, say what was tested and what was not.

## Branch workflow

UnixNotis uses two main branches:

- `dev` is the branch for active work
- `master` is the stable branch

New work should start on `dev`.

Before starting work, make sure local branches are current:

```sh
git fetch origin
git switch dev
git pull --ff-only origin dev
```

Before opening a pull request, check that the branch still contains the latest target branch.
Use `origin/dev` for normal contribution PRs, or `origin/master` for `dev -> master` release PRs:

```sh
git fetch origin
git merge-base --is-ancestor origin/dev HEAD
```

If that command fails, update from the target branch before opening the pull request:

```sh
git rebase origin/dev
```

If the branch is `dev` itself, prefer a fast-forward update instead:

```sh
git pull --ff-only origin dev
```

If the update is not clean, stop and resolve the branch state before adding more commits.

If a fix lands on `master`, bring `dev` back in sync before continuing work there.

This repository has a GitHub Actions workflow that handles that sync automatically:

- a push to `master` triggers a `master -> dev` sync attempt
- if the merge is clean, the workflow updates `dev`
- if the sync cannot be completed cleanly, the workflow opens follow-up work instead of failing quietly

## Pull requests

Please try to keep pull requests easy to review.

- Use a clear commit history
- Keep the PR focused on one problem or one closely related set of problems
- Explain the user-visible problem being solved
- Explain any behavior change, tradeoff, or limitation
- Include before/after measurements when claiming performance improvements
- Update docs when behavior or configuration changes

If docs live in the Wiki and are not updated in the PR, note what needs to be updated.

## Bug reports and fixes

If fixing a bug:

- Include the trigger or failure mode
- Include the root cause if it is known
- Prefer fixes that address the cause instead of hiding the symptom
- Add or update tests when practical

## AI-assisted contributions

AI-assisted contributions are allowed, but only within clear limits.

We recognize the usefulness and technological progress of AI models for software development. However, it is important to set boundaries for how those tools may and may not be used in UnixNotis contributions.

This section should be treated as a whitelist. If a use is not clearly acceptable here, assume it is not acceptable.

Acceptable uses include:

- Using AI locally to help understand the codebase or documentation
- Using AI locally to draft patches, tests, or documentation
- Using AI locally to help review or explain code before submission

Not acceptable:

- Fully autonomous submissions with no real human review
- AI-generated pull requests, issue reports, or review replies posted as-is
- AI-written developer-facing communication submitted without being checked and rewritten by the contributor
- Submitting code that is not understood by the person sending it

If AI is used, the contributor accepts full responsibility for the result. This means:

- The contributor has read, understood, and reviewed the generated code before submission
- The contributor has verified that the code is correct, in scope, and maintainable
- The contributor has checked for unnecessary complexity, regressions, and waste
- The contributor is responsible for licensing, attribution, and policy compliance for submitted content

AI output is not an excuse for low-quality, unreviewed, or unexplained changes. Contributions that ignore these rules may be rejected or closed.

Shout out to vaxerski on GitHub for the inspiration behind this section:
[hyprwm/.github AI usage policy](https://github.com/hyprwm/.github/blob/main/policies/AI_USAGE.md)

## Ways to support the project

if the project is useful, but there is no time to contribute code, that is fine too. There are other easy ways to support the project and show appreciation:
- Star the project
- Tweet or post about it
- Mention UnixNotis in another project's README
- Mention the project at local meetups
- Tell friends or coworkers who might find it useful
- Report bugs clearly and with enough detail to reproduce them

## Questions

If something is unclear, ask before making a large change.

Small, focused, well-tested contributions are much more likely to land cleanly than large rewrites.
