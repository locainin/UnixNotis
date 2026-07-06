# Changelog

## UnixNotis v1.1.1

### Highlights
- Added first-class widget asset icons through `icon_asset` for stat, toggle, and card widgets.
- Kept existing `icon` theme-name behavior as the fallback path, so old configs continue to load.
- Hardened widget asset paths for portable presets by requiring config-relative visual assets inside the UnixNotis config tree.
- Added stable widget-kind CSS hooks for stats and expanded CSS-check awareness for dynamic widget classes.
- Updated the wiki documentation for widget icons, preset portability, paths, configuration, and troubleshooting.

### Widget Icons
- Stat, toggle, and card widgets can now set `icon_asset = "assets/example.svg"` beside their existing `icon` fallback.
- Local icon assets are resolved from the config root instead of from process working directories.
- Invalid icon assets now warn and fall back to the configured theme icon instead of rendering GTK's broken-image placeholder.
- Supported asset extensions are limited to common image formats: `.svg`, `.png`, `.webp`, `.jpg`, and `.jpeg`.
- Asset validation rejects absolute paths, parent traversal, remote URLs, `file://` URLs, symlink escapes, directories, oversized files, and executable files.

### Presets and CSS Checks
- Preset import now validates widget `icon_asset` references before writing imported files.
- Presets can carry portable icon assets under their config tree without exposing host-specific paths.
- CSS-check now treats dynamic widget-kind hooks as known public classes.
- Stat widgets now expose sanitized classes such as `unixnotis-stat-kind-ram` for theme authors.

### Compatibility
- Existing configs that only use `icon = "theme-symbolic-name"` remain supported.
- `icon_asset` is optional and falls back to `icon` when missing or invalid.
- Widget kind class generation is sanitized and deterministic for unusual kind labels.

### Release Packaging
- Bumped workspace packages to `1.1.1`.
- Updated release packaging references for `scripts/package-release.sh v1.1.1`.
- Release archives continue to include `unixnotis-installer`, `unixnotis-release.json`, and bundled runtime binaries under `bin/`.
