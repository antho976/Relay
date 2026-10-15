#!/usr/bin/env python3
"""Register this native checkout's launcher and icon for the current user."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
launcher = root / "run.sh"
instance = sys.argv[1] if len(sys.argv) > 1 else "dev"
if instance not in ("dev", "test", "stable"):
    raise SystemExit("Expected dev, test, or stable")
data = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")
if not data.is_absolute():
    raise SystemExit("XDG_DATA_HOME must be an absolute path")

# Desktop Entry arguments are parsed without a shell. Escape field codes and
# reserved characters so a checkout path with spaces or punctuation stays one argv.
argument = str(launcher).replace("%", "%%")
for character in ("\\", '"', "`", "$"):
    argument = argument.replace(character, "\\" + character)
# The key file's string escapes are undone before the quoting above is read, so every
# backslash written so far has to survive that first pass as itself.
argument = argument.replace("\\", "\\\\")
template = root / "apps/relay-native/resources/com.quietsoftware.Relay4.desktop"
lines = template.read_text().splitlines(keepends=True)
execs = [at for at, line in enumerate(lines) if line.rstrip("\n") == "Exec=relay-native"]
if len(execs) != 1:
    raise SystemExit(f"{template}: expected exactly one 'Exec=relay-native' line, found {len(execs)}")
lines[execs[0]] = f'Exec="{argument}" {instance}\n'
desktop = "".join(lines)
assets = {
    data / "applications/com.quietsoftware.Relay4.desktop": desktop.encode(),
    data / "icons/hicolor/scalable/apps/com.quietsoftware.Relay4.svg":
        (root / "apps/relay-native/resources/com.quietsoftware.Relay4.svg").read_bytes(),
}
changed = False
for target, content in assets.items():
    target.parent.mkdir(parents=True, exist_ok=True)
    if not target.exists() or target.read_bytes() != content:
        # Written beside and renamed over, so the directory's mtime moves: icon loaders (Plasma's
        # task manager among them) only notice a theme change by its directories, and kept
        # showing an icon overwritten in place.
        staging = target.with_name(f".{target.name}.new")
        staging.write_bytes(content)
        staging.replace(target)
        changed = True
if changed:
    os.utime(data / "icons/hicolor")
    # KDE's service cache, where it has one; elsewhere the desktop picks the change up itself.
    for rebuild in ("kbuildsycoca6", "kbuildsycoca5"):
        if shutil.which(rebuild):
            subprocess.run([rebuild], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=60, check=False)
            break
