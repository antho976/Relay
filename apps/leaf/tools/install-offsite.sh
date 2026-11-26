#!/usr/bin/env bash
# Installs the nightly copy of Leaf's server data to this PC (pull-backup.sh) as a systemd user
# timer, 04:30 each night (caught up at the next start if the PC was off), and runs it once now.
# Undo: `systemctl --user disable --now leaf-offsite.timer` and delete the three files it names.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$HOME/.local/bin" "$HOME/.config/systemd/user"
install -m 755 "$here/pull-backup.sh" "$HOME/.local/bin/leaf-pull-backup.sh"
cat > "$HOME/.config/systemd/user/leaf-offsite.service" <<'EOF'
[Unit]
Description=Copy Leaf's server data to this PC (apps/leaf/tools/pull-backup.sh)
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/leaf-pull-backup.sh
EOF
cat > "$HOME/.config/systemd/user/leaf-offsite.timer" <<'EOF'
[Unit]
Description=Copy Leaf's server data to this PC nightly

[Timer]
OnCalendar=*-*-* 04:30
Persistent=true
RandomizedDelaySec=10min

[Install]
WantedBy=timers.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now leaf-offsite.timer
echo "Installed: ~/.local/bin/leaf-pull-backup.sh, ~/.config/systemd/user/leaf-offsite.{service,timer}"
"$HOME/.local/bin/leaf-pull-backup.sh"
