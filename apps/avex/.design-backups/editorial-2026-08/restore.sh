#!/bin/sh
# Undoes design/surface-experiment completely: Home, Profile, the forked kit, the gate exclusion
# and the allowlist paydown all go back to commit cdd50f2. See README.md. Run from anywhere:
#
#   sh .design-backups/editorial-2026-08/restore.sh
#
# --delete is deliberate: it removes files the experiment ADDED to the two packages
# (HomeSurfaceCards.kt, ProfileSurfaceSections.kt), which a plain copy would leave behind to
# reference a kit that no longer exists — a broken build rather than a restored one.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
UI="$REPO/forge-android/app/src/main/java/com/forge/app/ui"
TEST="$REPO/forge-android/app/src/test"

# 1. The two screens under test, and every file they gained.
rsync -a --delete "$HERE/src/overview/" "$UI/overview/"
rsync -a --delete "$HERE/src/profile/"  "$UI/profile/"

# 2. The forked card kit. Nothing outside the experiment ever imported it.
rm -rf "$UI/experiment"

# 3. The Roborazzi goldens (unchanged by this branch, restored for completeness).
cp "$HERE"/goldens/*.png "$TEST/screenshots/"

# 4. The gate: the branch-scoped SANCTIONED block, and the four allowlist lines the rewrite
#    paid down. Both go back to their cdd50f2 state, so the ratchet reads exactly as before.
cp "$HERE/gate/DesignDoctrine.kt"    "$TEST/java/com/forge/app/ui/DesignDoctrine.kt"
cp "$HERE/gate/design-allowlist.txt" "$TEST/resources/design-allowlist.txt"

echo "Restored: ui/overview, ui/profile, the goldens, and the design gate — all at cdd50f2."
echo "Removed:  ui/experiment (the forked card kit)."
echo "Untouched by this branch and so untouched here: ui/common, ui/theme, .claude/*."
echo "Check with: git status --short"
