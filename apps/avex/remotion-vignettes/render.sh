#!/usr/bin/env bash
# Render one onboarding plan-mode vignette to an alpha-transparent, infinitely-looping animated
# WebP and drop it into the Android app's res/raw. Usage: ./render.sh <composition-id>
# (composition ids: generated · custom · freestyle — see src/Root.tsx)
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.local/opt/node/bin:$PATH"

COMP="${1:?usage: ./render.sh <composition-id>}"
RAW_DIR="../forge-android/app/src/main/res/raw"

rm -rf "out/$COMP"
npx remotion render "src/index.ts" "$COMP" "out/$COMP" --sequence --image-format=png

ffmpeg -y -framerate 30 -i "out/$COMP/element-%03d.png" \
  -c:v libwebp_anim -lossless 0 -quality 78 -preset drawing -loop 0 -pix_fmt yuva420p \
  "$RAW_DIR/planmode_${COMP}.webp"

ls -la "$RAW_DIR/planmode_${COMP}.webp"
