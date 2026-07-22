#!/usr/bin/env bash
set -euo pipefail

# Unit-test modules need one declaration in production, but all test behavior stays under /tests
violations=""
while IFS= read -r -d '' file; do
    file_violations="$({
        awk '
            function report(line, message) {
                printf "%s:%d: %s\n", FILENAME, line, message
            }

            /#\[cfg\(test\)\]/ {
                pending_cfg = 1
                cfg_line = NR
                next
            }

            pending_cfg && /^[[:space:]]*($|\/\/|#\[)/ {
                next
            }

            pending_cfg {
                declaration = $0
                sub(/^[[:space:]]*/, "", declaration)
                if (declaration !~ /^(pub(\([^)]*\))?[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*;/) {
                    report(cfg_line, "test-only code must live under a /tests directory")
                }
                pending_cfg = 0
            }

            /#\[(tokio::|gtk::)?test\]/ {
                report(NR, "test body must live under a /tests directory")
            }

            /#\[cfg\(all\(test|#\[cfg_attr\([^]]*test|cfg!\(test\)/ {
                report(NR, "test-only condition must live under a /tests directory")
            }

            /(^|[^A-Za-z0-9_])([A-Za-z0-9_]+_for_test|test_override)([^A-Za-z0-9_]|$)/ {
                report(NR, "test-only helper must live under a /tests directory")
            }

            END {
                if (pending_cfg) {
                    report(cfg_line, "test module declaration is incomplete")
                }
            }
        ' "$file"
    } || true)"
    if [[ -n "$file_violations" ]]; then
        violations+="$file_violations"$'\n'
    fi
done < <(find crates -type f -path '*/src/*.rs' ! -path '*/tests/*' -print0)

# Nested modules keep their tests beside their own source directory
# Parent-level test paths make ownership unclear and leave stale folders after moves
while IFS=: read -r file line _match; do
    violations+="${file}:${line}: test module must use its source directory's /tests tree"$'\n'
done < <(
    rg --line-number --no-heading '#\[path[[:space:]]*=[[:space:]]*"(\.\./)+tests/' \
        crates -g '*.rs' || true
)

# Every test source below /src needs an incoming Rust module declaration
# This catches test files that look complete but Cargo never compiles
declare -A wired_test_files=()
while IFS= read -r -d '' source_file; do
    source_directory="$(dirname -- "$source_file")"
    source_name="$(basename -- "$source_file")"
    module_directory="$source_directory/${source_name%.rs}"
    case "$source_name" in
        lib.rs | main.rs | mod.rs)
            module_directory="$source_directory"
            ;;
    esac

    while IFS= read -r module_path; do
        wired_test_files["$(realpath -m -- "$source_directory/$module_path")"]=1
    done < <(
        sed -nE 's/^[[:space:]]*#\[path[[:space:]]*=[[:space:]]*"([^"]+)"\][[:space:]]*$/\1/p' \
            "$source_file"
    )

    while IFS= read -r module_name; do
        for candidate in \
            "$module_directory/$module_name.rs" \
            "$module_directory/$module_name/mod.rs"; do
            if [[ -f "$candidate" ]]; then
                wired_test_files["$(realpath -m -- "$candidate")"]=1
            fi
        done
    done < <(
        sed -nE \
            's/^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?mod[[:space:]]+(r#)?([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*;.*$/\4/p' \
            "$source_file"
    )
done < <(find crates -type f -path '*/src/*.rs' -print0)

while IFS= read -r -d '' test_file; do
    canonical_test_file="$(realpath -m -- "$test_file")"
    if [[ -z "${wired_test_files[$canonical_test_file]+present}" ]]; then
        violations+="${test_file}: test source is not wired into the Rust module graph"$'\n'
    fi
done < <(
    find crates -type f \
        \( -path '*/src/tests/*.rs' -o -path '*/src/*/tests/*.rs' \) \
        -print0
)

# A support module is test code too, even when it contains no #[test] function itself
while IFS= read -r path; do
    violations+="${path}: test support must live under a /tests directory"$'\n'
done < <(find crates -path '*/src/test_support*' -print)

if [[ -n "$violations" ]]; then
    printf 'test placement violations:\n%s' "$violations" >&2
    exit 1
fi

printf 'all test code is contained under mirrored /tests directories\n'
