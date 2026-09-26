#!/usr/bin/env bash
# Render onboarding plan-mode vignettes to alpha-transparent, infinitely-looping animated WebPs and
# drop them into the Android app's res/raw. `./render.sh all` does the set; `./render.sh <id>` does
# one (ids: generated · custom · freestyle — see src/Root.tsx).
#
# The card, not the file, decides how many times these play: PlanModeMedia.kt sets repeatCount so
# each one loops twice and then freezes on its last frame. `-loop 0` here just means "the file itself
# does not cap the count" — leave it alone.
#
# Cost note: soft gradients dominate the encoded size. Glows have been tried and cut twice: a bloom
# under the generated card's bars as they lock took that file from 147KB to 530KB, and one under each
# cell of an earlier freestyle design (a heatmap) as it popped took 309KB to 950KB, neither visible
# at 72dp. If you add bloom, re-check the size. Text that fades or moves is the other cost:
# freestyle's labels and custom's tilting tiles are why those two are three times the size of
# generated.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.local/opt/node/bin:$PATH"

RAW_DIR="../forge-android/app/src/main/res/raw"

render_one() {
  local comp="$1"
  mkdir -p "out/$comp"
  find "out/$comp" -name '*.png' -delete   # stale frames from a longer render would be picked up below
  npx remotion render "src/index.ts" "$comp" "out/$comp" --sequence --image-format=png

  ffmpeg -y -loglevel error -framerate 30 -i "out/$comp/element-%03d.png" \
    -c:v libwebp_anim -lossless 0 -quality 78 -preset drawing -compression_level 6 \
    -loop 0 -pix_fmt yuva420p \
    "$RAW_DIR/planmode_${comp}.webp"

  ls -la "$RAW_DIR/planmode_${comp}.webp"
}

if [ "${1:?usage: ./render.sh <composition-id>|all}" = all ]; then
  for comp in generated custom freestyle; do render_one "$comp"; done
else
  render_one "$1"
fi
