#!/usr/bin/env bash

set -euo pipefail

main() {
  local tag="${1:-}"
  if [[ -z "$tag" ]]; then
    printf 'usage: %s v1.0.0\n' "${0}" >&2
    exit 2
  fi

  if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    # Release artifacts and installer update checks both expect stable vMAJOR.MINOR.PATCH tags
    printf 'release tag must look like v1.0.0: %s\n' "$tag" >&2
    exit 2
  fi

  local version="${tag#v}"
  local target="x86_64-unknown-linux-gnu"
  local root
  root="$(repo_root)"
  cd "$root"

  assert_workspace_version "$version"
  # Build first so packaging never creates an archive around stale target artifacts
  build_release_binaries
  assemble_archive "$tag" "$version" "$target"
}

repo_root() {
  # Resolve through git so the script can run from any subdirectory
  git rev-parse --show-toplevel
}

assert_workspace_version() {
  local expected="${1}"
  local pkgid
  local actual

  # cargo pkgid reads Cargo metadata and avoids hand-parsing Cargo.toml
  pkgid="$(cargo pkgid -p unixnotis-installer)"
  actual="${pkgid##*#}"

  if [[ "$actual" != "$expected" ]]; then
    printf 'Cargo version %s does not match release tag v%s\n' "$actual" "$expected" >&2
    exit 1
  fi
}

build_release_binaries() {
  # Build only the programs the installer deploys plus the installer itself
  cargo build --release \
    -p unixnotis-installer \
    -p unixnotis-daemon \
    -p unixnotis-popups \
    -p unixnotis-center \
    -p noticenterctl
}

assemble_archive() {
  local tag="${1}"
  local version="${2}"
  local target="${3}"
  local package_root="unixnotis-${tag}-${target}"
  local dist_root="dist/${package_root}"
  local archive="dist/${package_root}.tar.zst"

  # Start from a clean package directory so stale binaries cannot leak into a release
  # The dist directory itself is ignored so local package checks do not dirty commits
  rm -rf "$dist_root" "$archive" "${archive}.sha256"
  mkdir -p "${dist_root}/bin"

  # The top-level installer finds unixnotis-release.json beside itself at runtime
  install -m 0755 target/release/unixnotis-installer "${dist_root}/unixnotis-installer"

  # Runtime tools stay under bin so the installer can validate and copy them as a group
  install -m 0755 target/release/unixnotis-daemon "${dist_root}/bin/unixnotis-daemon"
  install -m 0755 target/release/unixnotis-popups "${dist_root}/bin/unixnotis-popups"
  install -m 0755 target/release/unixnotis-center "${dist_root}/bin/unixnotis-center"
  install -m 0755 target/release/noticenterctl "${dist_root}/bin/noticenterctl"

  write_manifest "${dist_root}/unixnotis-release.json" "$tag" "$version" "$target"
  write_readme "${dist_root}/README.txt" "$tag"

  # zstd keeps the release small while still being standard on modern Linux systems
  tar --zstd -C dist -cf "$archive" "$package_root"
  # The checksum file is uploaded with the archive for a simple manual verification path
  sha256sum "$archive" > "${archive}.sha256"

  printf 'wrote %s\n' "$archive"
  printf 'wrote %s\n' "${archive}.sha256"
}

write_manifest() {
  local path="${1}"
  local tag="${2}"
  local version="${3}"
  local target="${4}"

  # Keep this schema tiny because the installer trusts it to find bundled binaries
  printf '{"version":"%s","tag":"%s","target":"%s","binaries":["unixnotis-daemon","unixnotis-popups","unixnotis-center","noticenterctl"]}\n' \
    "$version" \
    "$tag" \
    "$target" \
    > "$path"
}

write_readme() {
  local path="${1}"
  local tag="${2}"

  # The archive README is intentionally short because the TUI owns the real install flow
  printf '%s\n' \
    "UnixNotis ${tag}" \
    "" \
    "Run ./unixnotis-installer from this directory to install bundled binaries." \
    "The installer writes user-level files only and supports systemd, dinit, runit, and s6." \
    > "$path"
}

main "$@"
