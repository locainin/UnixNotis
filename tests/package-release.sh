#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "${script_dir}/.." && pwd -P)"
cd -- "$repo_root"
source scripts/package-release.sh

if ! managed_binaries | grep -Fxq 'unixnotis-svg-renderer'; then
  printf 'installer metadata omitted the SVG renderer\n' >&2
  exit 1
fi

test_root="$(mktemp -d)"
trap 'rm -rf -- "${test_root}"' EXIT
cd -- "$test_root"

# Fake release binaries keep this regression fast while exercising the real archive assembly
mkdir -p target/release
printf 'installer\n' > target/release/unixnotis-installer
printf 'daemon\n' > target/release/unixnotis-daemon
printf 'center\n' > target/release/unixnotis-center
printf 'svg-renderer\n' > target/release/unixnotis-svg-renderer
chmod 0755 target/release/*

export SOURCE_DATE_EPOCH=1700000000
assemble_archive v9.8.7 9.8.7 x86_64-unknown-linux-gnu unixnotis-daemon unixnotis-center unixnotis-svg-renderer
archive="dist/unixnotis-v9.8.7-x86_64-unknown-linux-gnu.tar.zst"
first_digest="$(sha256sum "$archive" | cut -d ' ' -f 1)"

# Input timestamps must not influence the published archive
touch target/release/*
assemble_archive v9.8.7 9.8.7 x86_64-unknown-linux-gnu unixnotis-daemon unixnotis-center unixnotis-svg-renderer
second_digest="$(sha256sum "$archive" | cut -d ' ' -f 1)"

if [[ "$first_digest" != "$second_digest" ]]; then
  printf 'release archive changed despite identical input bytes\n' >&2
  exit 1
fi

expected_checksum="${second_digest}  $(basename -- "$archive")"
actual_checksum="$(cat -- "${archive}.sha256")"
if [[ "$actual_checksum" != "$expected_checksum" ]]; then
  printf 'release checksum does not use the downloadable archive name\n' >&2
  exit 1
fi

if ! tar --zstd --numeric-owner -tvf "$archive" | awk '{print $2}' | grep -qx '0/0'; then
  printf 'release archive ownership is not normalized\n' >&2
  exit 1
fi

(
  cd -- "$(dirname -- "$archive")"
  sha256sum -c "$(basename -- "${archive}.sha256")"
)

# The CSS validator proves that an installed binary name can differ from its Cargo package
cargo_args="${test_root}/cargo-args"
cargo() {
  printf '%s\n' "$@" > "$cargo_args"
}
build_release_binaries unixnotis-daemon unixnotis-svg-renderer unixnotis-css-validate
unset -f cargo

expected_args=$'build\n--locked\n--release\n--bin\nunixnotis-installer\n--bin\nunixnotis-daemon\n--bin\nunixnotis-svg-renderer\n--bin\nunixnotis-css-validate'
actual_args="$(cat -- "$cargo_args")"
if [[ "$actual_args" != "$expected_args" ]]; then
  printf 'release build did not select exact binary targets\n' >&2
  exit 1
fi
