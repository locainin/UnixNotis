#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "${script_dir}/.." && pwd -P)"
workflow="${repo_root}/.github/workflows/release.yml"
packager="${repo_root}/scripts/package-release.sh"

assert_contains() {
  local path="${1}"
  local expected="${2}"

  # Fixed-string matching keeps workflow syntax out of regular-expression parsing
  if ! grep -Fq -- "$expected" "$path"; then
    printf 'missing release hardening in %s: %s\n' "$path" "$expected" >&2
    return 1
  fi
}

assert_excludes() {
  local path="${1}"
  local rejected="${2}"

  # Mutable installers and live Cargo tools must not enter the release builder
  if grep -Fq -- "$rejected" "$path"; then
    printf 'mutable release input remains in %s: %s\n' "$path" "$rejected" >&2
    return 1
  fi
}

assert_count() {
  local path="${1}"
  local expected_count="${2}"
  local needle="${3}"
  local actual_count

  actual_count="$(grep -Fc -- "$needle" "$path")"
  if [[ "$actual_count" != "$expected_count" ]]; then
    printf 'expected %s occurrences in %s, found %s: %s\n' \
      "$expected_count" "$path" "$actual_count" "$needle" >&2
    return 1
  fi
}

# The base image and package repository both resolve to immutable inputs
assert_contains "$workflow" 'container: debian:trixie-slim@sha256:'
assert_contains "$workflow" "snapshot.debian.org/archive/debian/\${DEBIAN_SNAPSHOT}"
assert_contains "$workflow" "snapshot.debian.org/archive/debian-security/\${DEBIAN_SNAPSHOT}"

# Rustup is downloaded from a versioned archive and checked before execution
assert_contains "$workflow" 'RUSTUP_INIT_VERSION: 1.28.2'
assert_contains "$workflow" 'RUSTUP_INIT_SHA256: 20a06e644b0d9bd2fbdbfd52d42540bdde820ea7df86e92e533c073da0cdd43c'
assert_contains "$workflow" '| sha256sum --check --strict'
assert_excludes "$workflow" 'https://sh.rustup.rs'
assert_excludes "$workflow" 'cargo install'

# A release input must select the same tag and commit that triggered the workflow
# Literal workflow expressions must stay unexpanded in these fixed-string checks
# shellcheck disable=SC2016
assert_contains "$workflow" 'if [[ "$GITHUB_REF" != "refs/tags/${RELEASE_TAG}" ]]; then'
# shellcheck disable=SC2016
assert_contains "$workflow" 'tag_commit="$(git rev-parse "${tag_ref}^{commit}")"'
# shellcheck disable=SC2016
assert_contains "$workflow" 'if [[ "$tag_commit" != "$GITHUB_SHA" || "$checked_out_commit" != "$GITHUB_SHA" ]]; then'

# Release builds cannot update the dependency lockfile
assert_contains "$packager" 'local args=(build --locked --release'
assert_contains "$packager" 'cargo pkgid --locked'
assert_contains "$packager" 'cargo metadata --locked --no-deps'

# Archives and checksum manifests receive both portable signatures and provenance
assert_contains "$workflow" 'sigstore/cosign-installer@ba7bc0a3fef59531c69a25acd34668d6d3fe6f22'
assert_contains "$workflow" 'cosign-release: v3.1.2'
assert_contains "$workflow" 'cosign sign-blob'
assert_contains "$workflow" 'actions/attest@f7c74d28b9d84cb8768d0b8ca14a4bac6ef463e6'
assert_contains "$workflow" 'dist/*.sigstore.json'

# Build steps cannot mint identities; only the dependent signing job receives OIDC
assert_contains "$workflow" 'needs: package'
assert_contains "$workflow" 'actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c'
# shellcheck disable=SC2016
assert_contains "$workflow" 'name: unixnotis-${{ inputs.tag }}-unsigned'
assert_count "$workflow" 1 'id-token: write'
assert_count "$workflow" 1 'attestations: write'
assert_count "$workflow" 1 'artifact-metadata: write'
