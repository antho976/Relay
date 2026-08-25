#!/usr/bin/env bash
# Capture a screen recording from a connected device and land it in public/cfr/ ready to cut.
#
#   tools/capture.sh after-notifications 25
#
# `adb screenrecord` writes VARIABLE frame rate — these takes have averaged ~56fps — and a frame
# index measured with ffmpeg then means a different instant to Remotion, which counts composition
# frames at 30. Everything in public/cfr/ is transcoded to constant 30fps so the two agree; that
# transcode is the whole reason this script exists rather than a bare `adb pull`.
set -euo pipefail

NAME="${1:?usage: capture.sh <name> [seconds]}"
SECS="${2:-20}"
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$DIR/public/cfr/$NAME.mp4"

# The existing captures are all 1080x2400; matching that keeps one set of focus offsets valid.
adb shell wm size | grep -q '1080x2400' || echo "note: device is not at 1080x2400 — run: adb shell wm size 1080x2400"

echo "recording ${SECS}s — drive the phone now"
adb shell screenrecord --size 1080x2400 --bit-rate 16000000 --time-limit "$SECS" /sdcard/_cap.mp4
adb pull /sdcard/_cap.mp4 /tmp/_cap.mp4 >/dev/null
adb shell rm /sdcard/_cap.mp4

ffmpeg -v error -y -i /tmp/_cap.mp4 -vsync cfr -r 30 -c:v libx264 -crf 18 -preset slow -pix_fmt yuv420p -an "$OUT"
rm -f /tmp/_cap.mp4

FRAMES=$(ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 "$OUT")
echo "$OUT — $FRAMES frames at 30fps ($(echo "scale=1; $FRAMES/30" | bc)s)"
echo "contact sheet:"
ffmpeg -v error -y -i "$OUT" -vf "select='not(mod(n,$((FRAMES/6))))',scale=200:444,tile=6x1" -frames:v 1 "$DIR/out/$NAME-sheet.png"
echo "  $DIR/out/$NAME-sheet.png"
