#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

# UnixNotis installer for the daemon and popup UI with systemd --user.
# This script builds release binaries, installs them to $HOME/.local/bin,
# configures a user service, and switches ownership from a detected daemon.
# This is a work in progress and may not work
readonly SERVICE_NAME="unixnotis-daemon.service"
readonly SERVICE_DIR="$HOME/.config/systemd/user"
readonly BIN_DIR="$HOME/.local/bin"

readonly KNOWN_DAEMONS=(
  "mako:mako.service"
  "dunst:dunst.service"
  "swaync:swaync.service"
  "notify-osd:notify-osd.service"
)

show_usage() {
  cat <<'USAGE'
Usage: scripts/install.sh [--yes] [--skip-checks]

Options:
  --yes          Skip confirmation prompts
  --skip-checks  Skip cargo check/test verification
USAGE
}

YES=0
SKIP_CHECKS=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --yes)
      YES=1
      shift
      ;;
    --skip-checks)
      SKIP_CHECKS=1
      shift
      ;;
    -h|--help)
      show_usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      show_usage
      exit 1
      ;;
  esac
done

if [[ ! -f Cargo.toml ]]; then
  echo "Run from the repository root where Cargo.toml is present." >&2
  exit 1
fi

require_command() {
  local name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    echo "Missing required command: $name" >&2
    exit 1
  fi
}

require_command cargo
require_command systemctl

if ! systemctl --user show-environment >/dev/null 2>&1; then
  echo "systemd --user is not available in this session." >&2
  exit 1
fi

detect_owner() {
  if command -v busctl >/dev/null 2>&1; then
    local status
    status="$(busctl --user status org.freedesktop.Notifications 2>/dev/null || true)"
    OWNER_COMM="$(printf '%s\n' "$status" | awk -F= '/^Comm=/{print $2}' | tr -d '\r')"
    OWNER_PID="$(printf '%s\n' "$status" | awk -F= '/^PID=/{print $2}' | tr -d '\r')"
  else
    OWNER_COMM=""
    OWNER_PID=""
  fi
}

is_unit_active() {
  local unit="$1"
  systemctl --user is-active --quiet "$unit"
}

pgrep_exact() {
  local name="$1"
  pgrep -x "$name" 2>/dev/null || true
}

print_detection() {
  echo "Detected notification daemons:"
  for entry in "${KNOWN_DAEMONS[@]}"; do
    local daemon="${entry%%:*}"
    local unit="${entry##*:}"
    local status=()
    if [[ -n "${OWNER_COMM:-}" && "$OWNER_COMM" == "$daemon" ]]; then
      status+=("dbus-owner")
    fi
    if is_unit_active "$unit"; then
      status+=("systemd-active")
    fi
    local pids
    pids="$(pgrep_exact "$daemon")"
    if [[ -n "$pids" ]]; then
      status+=("pid $(printf '%s' "$pids" | tr '\n' ',' | sed 's/,$//')")
    fi
    if [[ ${#status[@]} -eq 0 ]]; then
      status+=("not running")
    fi
    echo "- ${daemon}: ${status[*]}"
  done
  if [[ -n "${OWNER_COMM:-}" && -n "${OWNER_PID:-}" ]]; then
    echo "Current owner: ${OWNER_COMM} (pid ${OWNER_PID})"
  fi
}

confirm_switch() {
  if [[ $YES -eq 1 ]]; then
    return 0
  fi
  printf "Switch default notifications to UnixNotis? [y/N]: "
  read -r reply
  case "$reply" in
    y|Y|yes|YES) return 0 ;;
    *) return 1 ;;
  esac
}

stop_existing_daemon() {
  if [[ -z "${OWNER_COMM:-}" ]]; then
    return 0
  fi
  for entry in "${KNOWN_DAEMONS[@]}"; do
    local daemon="${entry%%:*}"
    local unit="${entry##*:}"
    if [[ "$OWNER_COMM" == "$daemon" ]]; then
      if is_unit_active "$unit"; then
        systemctl --user disable --now "$unit"
        return 0
      fi
      if [[ -n "${OWNER_PID:-}" ]]; then
        kill -TERM "$OWNER_PID" 2>/dev/null || true
        return 0
      fi
    fi
  done
  if [[ -n "${OWNER_COMM:-}" ]]; then
    echo "Unknown notification daemon detected (${OWNER_COMM}); stopping it is not automated." >&2
    exit 1
  fi
}

run_checks() {
  if [[ $SKIP_CHECKS -eq 1 ]]; then
    return 0
  fi
  run_script_checks
  # Enforce warning-free builds and run unit tests before installation.
  RUSTFLAGS="-D warnings" cargo check
  RUSTFLAGS="-D warnings" cargo test
  run_clippy_checks
}

install_binaries() {
  # Build the CLI so Waybar keybinds can invoke the panel and DND toggles.
  cargo build --release -p unixnotis-daemon -p unixnotis-popups -p unixnotis-center -p noticenterctl
  install -d "$BIN_DIR"
  install -Dm755 target/release/unixnotis-daemon "$BIN_DIR/unixnotis-daemon"
  install -Dm755 target/release/unixnotis-popups "$BIN_DIR/unixnotis-popups"
  install -Dm755 target/release/unixnotis-center "$BIN_DIR/unixnotis-center"
  install -Dm755 target/release/noticenterctl "$BIN_DIR/noticenterctl"
}

write_service() {
  install -d "$SERVICE_DIR"
  cat >"$SERVICE_DIR/$SERVICE_NAME" <<'UNIT'
[Unit]
Description=UnixNotis Notification Daemon
After=graphical-session.target
Wants=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/unixnotis-daemon
Restart=on-failure
RestartSec=1

[Install]
WantedBy=default.target
UNIT
  verify_service_unit
}

run_script_checks() {
  # Validate shell syntax and quoting practices when supporting tools are present.
  if command -v bash >/dev/null 2>&1; then
    bash -n "$0"
  fi
  if command -v shellcheck >/dev/null 2>&1; then
    shellcheck "$0"
  else
    echo "shellcheck not found; skipping shell lint."
  fi
  if command -v shellharden >/dev/null 2>&1; then
    shellharden --check "$0"
  else
    echo "shellharden not found; skipping quoting check."
  fi
}

run_clippy_checks() {
  # Include clippy perf lints when available to catch expensive patterns.
  if cargo clippy --version >/dev/null 2>&1; then
    cargo clippy --all-targets --all-features -- -D warnings -W clippy::perf
  else
    echo "cargo clippy not found; skipping perf lint."
  fi
}

verify_service_unit() {
  # systemd-analyze verify catches ordering issues and deprecated directives.
  if command -v systemd-analyze >/dev/null 2>&1; then
    systemd-analyze --user verify "$SERVICE_DIR/$SERVICE_NAME"
  else
    echo "systemd-analyze not found; skipping unit verification."
  fi
}

detect_owner
print_detection

if ! confirm_switch; then
  echo "Install cancelled."
  exit 0
fi

stop_existing_daemon
run_checks
install_binaries
write_service

systemctl --user daemon-reload
systemctl --user enable --now "$SERVICE_NAME"

echo "UnixNotis daemon installed and started."
echo "The popups and panel UI are started automatically by the daemon."
