#!/usr/bin/env bash
# Build and install the SLX LCD desktop app for the current user.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"

command -v cargo >/dev/null || { echo "Rust/Cargo is required." >&2; exit 1; }

printf '%s\n' '==> Building SLX LCD desktop app'
(cd "$ROOT/app/src-tauri" && cargo build --release)
printf '%s\n' '==> Building SLX LCD daemon'
(cd "$ROOT" && cargo build --release -p slxd)

install -Dm755 "$ROOT/app/src-tauri/target/release/slx-lcd-app" "$BIN_DIR/slx-lcd-app"
install -Dm755 "$ROOT/target/release/slxd" "$BIN_DIR/slxd"
mkdir -p "$APP_DIR"
sed "s|/usr/bin/env slx-lcd-app|$BIN_DIR/slx-lcd-app|" "$ROOT/packaging/slx-lcd.desktop" > "$APP_DIR/slx-lcd.desktop"
chmod 644 "$APP_DIR/slx-lcd.desktop"

CFG="${XDG_CONFIG_HOME:-$HOME/.config}/slxd/themes"
mkdir -p "$CFG"
cp -n "$ROOT"/themes/*.json "$CFG/" 2>/dev/null || true

if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$APP_DIR" || true
fi

printf '\n%s\n' 'SLX LCD installed. Open the application menu and search for “SLX LCD”.' \
  'If the LCD device is inaccessible, run packaging/install.sh once to install the udev rule.' \
  'Stop any other program using the LCD serial port before starting the app.'
