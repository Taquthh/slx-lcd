#!/usr/bin/env bash
# Install slxd for the current user. Only the udev rule needs sudo.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CFG="${XDG_CONFIG_HOME:-$HOME/.config}/slxd"

echo "==> building"
(cd "$ROOT" && cargo build --release -p slxd)

echo "==> installing binary and themes"
install -Dm755 "$ROOT/target/release/slxd" "$HOME/.local/bin/slxd"
mkdir -p "$CFG/themes"
cp -n "$ROOT"/themes/*.json "$CFG/themes/" || true

echo "==> installing systemd user service"
install -Dm644 "$ROOT/packaging/slxd.service" "$HOME/.config/systemd/user/slxd.service"
systemctl --user daemon-reload

echo "==> installing udev rule (sudo)"
sudo install -Dm644 "$ROOT/packaging/99-slx-lcd.rules" /etc/udev/rules.d/99-slx-lcd.rules
sudo udevadm control --reload
sudo udevadm trigger --subsystem-match=tty

cat <<MSG

Done. Next:
  systemctl --user enable --now slxd     # start now and at every login
  journalctl --user -u slxd -f           # watch the log
Stop any running turing-smart-screen-python (main.py) first: only one program can own the port.
MSG
