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

# A support module is test code too, even when it contains no #[test] function itself
while IFS= read -r path; do
    violations+="${path}: test support must live under a /tests directory"$'\n'
done < <(find crates -path '*/src/test_support*' -print)

if [[ -n "$violations" ]]; then
    printf 'test placement violations:\n%s' "$violations" >&2
    exit 1
fi

printf 'all test code is contained under mirrored /tests directories\n'
