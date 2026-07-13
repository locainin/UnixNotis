#!/usr/bin/env bash

set -euo pipefail

main() {
  local tag="${1:-}"
  if [[ -z "$tag" ]]; then
    printf 'usage: %s v1.1.0\n' "${0}" >&2
    exit 2
  fi

  if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    # Release artifacts and installer update checks both expect stable vMAJOR.MINOR.PATCH tags
    printf 'release tag must look like v1.1.0: %s\n' "$tag" >&2
    exit 2
  fi

  local version="${tag#v}"
  local target="x86_64-unknown-linux-gnu"
  local root
  local binaries=()
  local binary_list
  root="$(repo_root)"
  cd "$root"
  binary_list="$(managed_binaries)"
  readarray -t binaries <<< "$binary_list"

  assert_workspace_version "$version"
  # Build first so packaging never creates an archive around stale target artifacts
  build_release_binaries "${binaries[@]}"
  assemble_archive "$tag" "$version" "$target" "${binaries[@]}"
}

repo_root() {
  local script_dir

  # Resolve from this script so CI workspaces do not depend on git safe.directory state
  script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
  cd -- "${script_dir}/.."
  pwd -P
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
  local binaries=("$@")
  local args=(build --release -p unixnotis-installer)

  for binary in "${binaries[@]}"; do
    args+=(-p "$binary")
  done

  # Build only the programs the installer deploys plus the installer itself
  cargo "${args[@]}"
}

assemble_archive() {
  local tag="${1}"
  local version="${2}"
  local target="${3}"
  shift 3
  local binaries=("$@")
  local package_root="unixnotis-${tag}-${target}"
  local dist_root="dist/${package_root}"
  local archive="dist/${package_root}.tar.zst"
  local release_epoch
  release_epoch="$(release_epoch)"

  # Start from a clean package directory so stale binaries cannot leak into a release
  # The dist directory itself is ignored so local package checks do not dirty commits
  rm -rf "$dist_root" "$archive" "${archive}.sha256"
  mkdir -p "${dist_root}/bin"

  # The top-level installer finds unixnotis-release.json beside itself at runtime
  install -m 0755 target/release/unixnotis-installer "${dist_root}/unixnotis-installer"

  # Runtime tools stay under bin so the installer can validate and copy them as a group
  for binary in "${binaries[@]}"; do
    install -m 0755 "target/release/${binary}" "${dist_root}/bin/${binary}"
  done

  write_manifest "${dist_root}/unixnotis-release.json" "$tag" "$version" "$target" "${binaries[@]}"
  write_readme "${dist_root}/README.txt" "$tag"

  # Stable metadata makes the same inputs produce the same release bytes on every builder
  tar \
    --zstd \
    --sort=name \
    --format=gnu \
    --mtime="@${release_epoch}" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    -C dist \
    -cf "$archive" \
    "$package_root"
  # The checksum file is uploaded with the archive for a simple manual verification path
  (
    cd -- "$(dirname -- "$archive")"
    sha256sum "$(basename -- "$archive")"
  ) > "${archive}.sha256"

  printf 'wrote %s\n' "$archive"
  printf 'wrote %s\n' "${archive}.sha256"
}

release_epoch() {
  local epoch="${SOURCE_DATE_EPOCH:-0}"

  if [[ ! "$epoch" =~ ^[0-9]+$ ]]; then
    printf 'SOURCE_DATE_EPOCH must be an unsigned integer: %s\n' "$epoch" >&2
    return 2
  fi
  printf '%s\n' "$epoch"
}

write_manifest() {
  local path="${1}"
  local tag="${2}"
  local version="${3}"
  local target="${4}"
  shift 4
  local binaries=("$@")
  local json_binaries="["
  local separator=""

  for binary in "${binaries[@]}"; do
    json_binaries+="${separator}\"${binary}\""
    separator=","
  done
  json_binaries+="]"

  # Keep this schema tiny because the installer trusts it to find bundled binaries
  printf '{"version":"%s","tag":"%s","target":"%s","binaries":%s}\n' \
    "$version" \
    "$tag" \
    "$target" \
    "$json_binaries" \
    > "$path"
}

managed_binaries() {
  cargo metadata --no-deps --format-version 1 |
    python3 -c '
import json
import sys

metadata = json.load(sys.stdin)
binaries = (
    metadata.get("metadata", {})
    .get("unixnotis", {})
    .get("installer", {})
    .get("binaries", [])
)
if not isinstance(binaries, list) or not binaries:
    raise SystemExit("workspace metadata must define unixnotis.installer.binaries")

seen = set()
for raw in binaries:
    if not isinstance(raw, str):
        raise SystemExit("installer binary names must be strings")
    name = raw.strip()
    if not name or name in {".", ".."} or "/" in name or "\\" in name or "\"" in name:
        raise SystemExit(f"unsafe installer binary name: {raw!r}")
    if name not in seen:
        seen.add(name)
        print(name)
'
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

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
