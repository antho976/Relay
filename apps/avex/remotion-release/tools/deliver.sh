#!/usr/bin/env bash
# Assemble a deliverable from a rendered picture and a separately rendered soundtrack.
#
#   tools/deliver.sh out/avex-0.9-picture.mp4 out/avex-0.9.wav out/avex-0.9.mp4
#
# Why not just `remotion render`'s own mp4: its AAC track carries 2048 samples of encoder priming
# (42.67 ms at 48 kHz) that the container does not declare, so every player starts the sound 1.3
# frames after the picture. Measured by cross-correlating the rendered file against the bed
# (0.993 correlation at +42.3 ms) and against impact.wav at the onboarding hit (+42.9 ms). Rendering
# the soundtrack as WAV (`--codec=wav`) gives a track with no priming at all; muxing it here with
# ffmpeg's own AAC encoder writes the edit list that compensating players honour.
#
# AUDIO_LEAD_MS: if a future measurement of the muxed file still shows a lag, put it here — positive
# pulls the sound earlier. With the WAV path it is 0.
set -euo pipefail
PIC="${1:?picture mp4}"; WAV="${2:?soundtrack wav}"; OUT="${3:?output mp4}"
AUDIO_LEAD_MS="${AUDIO_LEAD_MS:-0}"
ffmpeg -y -v error -i "$PIC" -itsoffset "-$(awk -v m="$AUDIO_LEAD_MS" 'BEGIN{printf "%.6f", m/1000}')" -i "$WAV" \
  -map 0:v:0 -map 1:a:0 -c:v copy -c:a aac -b:a 320k -ar 48000 -movflags +faststart -shortest "$OUT"
ffprobe -v error -show_entries format=duration:stream=codec_name,width,height -of compact "$OUT"
ls -la "$OUT"
