#!/usr/bin/env bash
#
# make-sfx.sh — regenerate the UI sound palette for the release video.
#
# Everything here is synthesised from scratch by ffmpeg (sine / aevalsrc /
# anoisesrc). No downloads, no sample libraries, no network. Running this
# script twice produces byte-identical files: every noise source is seeded,
# and level setting is a measure-then-apply-exact-gain pass rather than a
# dynamics processor.
#
# Output: remotion-release/public/sfx/*.wav — 48 kHz, stereo, 16-bit PCM.
#
# LEVELS. Every sound is peak-normalised to the SAME target, -3 dBFS, via
# `finish <name> -3`. Keep it that way when adding sounds.
#
# The perceived hierarchy — which cue is loud, which sits back — lives entirely
# in src/Sound.tsx's LEVEL map. It used to be encoded here as well (tap -14,
# confirm -10, count -20, ...) and every cue was consequently attenuated twice.
# One source of truth: this script makes each file as loud as it can cleanly be,
# and the timeline decides how loud it should actually sound.
#
# Note that equal peak still does not mean equal loudness: the transients
# (tap/tick/count/pop) carry a ~22-24 dB peak-to-momentary-loudness crest while
# the sustained tonal cues carry only ~6-9 dB. That is a property of the sounds,
# not of the normalisation, and the LEVEL map is the right place to correct it.
#
# Usage (from anywhere, including the repo root):
#   ./remotion-release/tools/make-sfx.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$SCRIPT_DIR/../public/sfx"
mkdir -p "$OUT_DIR"
OUT_DIR="$(cd -- "$OUT_DIR" && pwd)"

command -v ffmpeg  >/dev/null || { echo "make-sfx: ffmpeg not found on PATH" >&2; exit 1; }
command -v ffprobe >/dev/null || { echo "make-sfx: ffprobe not found on PATH" >&2; exit 1; }

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

SR=48000

# ---------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------

# Build an intermediate 32-bit float working file from a filter_complex graph.
# Working in float means the envelope/mix maths never quantises twice; the
# single 16-bit conversion happens in finish().
#
# Each sound is synthesised mono and widened with an explicit unity-gain pan.
# That is deliberate: ffmpeg's automatic mono->stereo rematrix applies a -3 dB
# pan law, which would silently undershoot every level target set below.
#   synth <name> <filter_complex ending in [out]>
synth() {
  local name="$1" graph="$2"
  ffmpeg -y -hide_banner -loglevel error \
    -filter_complex "$graph;[out]pan=stereo|c0=c0|c1=c0[st]" -map '[st]' \
    -c:a pcm_f32le -ar "$SR" "$TMP_DIR/$name.wav"
}

# Highest "Peak level dB" astats reports across channels (i.e. true sample peak).
peak_dbfs() {
  ffmpeg -hide_banner -nostats -i "$1" -af astats=metadata=1 -f null - 2>&1 \
    | sed -n 's/.*Peak level dB: //p' | sort -g | tail -n1
}

# Measure the working file's peak, apply the exact linear gain that lands it on
# the requested target, and write the deliverable as 48 kHz / stereo / s16.
# Deterministic and idempotent — no limiter, no loudnorm two-pass drift.
#   finish <name> <target_peak_dbfs>
finish() {
  local name="$1" target="$2" src="$TMP_DIR/$1.wav" peak gain
  peak="$(peak_dbfs "$src")"
  gain="$(awk -v p="$peak" -v t="$target" 'BEGIN { printf "%.6f", t - p }')"
  ffmpeg -y -hide_banner -loglevel error -i "$src" \
    -af "volume=${gain}dB" \
    -ar "$SR" -ac 2 -c:a pcm_s16le "$OUT_DIR/$name.wav"
  printf '  %-14s %6.1f dBFS peak\n' "$name.wav" "$(peak_dbfs "$OUT_DIR/$name.wav")"
}

echo "make-sfx: writing to $OUT_DIR"

# ---------------------------------------------------------------------------
# 1. tap.wav — soft UI tap, ~60 ms. Fingertip on glass.
#
#    [a] a white-noise burst pushed through two 2-pole lowpasses at 1.8 kHz
#        (24 dB/oct — the dull "contact" of skin, not the bright snap of a
#        mouse switch) with the low mud highpassed off at 250 Hz, then an
#        exponential fade-out so the burst is essentially over in 20 ms.
#        White rather than pink: pink's own -3 dB/oct tilt stacked on top of
#        the lowpass left nothing above 1.6 kHz, and the tap lost its contact
#        definition and read as a pure tone instead.
#    [b] a QUIET 560 Hz sine pip, 1.5 ms attack, 9 ms decay — the small pitched
#        "body" under the click. It must stay UNDER the noise, and that takes a
#        much lower gain than it looks like it should: a sustained sine carries
#        far more RMS than a burst of noise that has already decayed, so at 0.55
#        (and even at 0.30) the pip was still louder than the noise and the tap
#        read as a pitched boop. At 0.11 the noise leads the pip by ~4 dB, which
#        is what makes it a tap with a hint of pitch rather than a tone.
#    Mixed 1.0 : 0.11, 2 ms anti-click fade-in, 8 ms tail fade-out.
# ---------------------------------------------------------------------------
synth tap "
  anoisesrc=color=white:amplitude=0.9:duration=0.06:sample_rate=$SR:seed=101
    ,highpass=frequency=250:poles=2
    ,lowpass=frequency=1800:poles=2
    ,lowpass=frequency=1800:poles=2
    ,afade=type=out:start_time=0.002:duration=0.056:curve=exp
    ,volume=1.0[a];
  aevalsrc=exprs='sin(2*PI*560*t)*(1-exp(-t/0.0015))*exp(-t/0.009)':sample_rate=$SR:duration=0.06
    ,volume=0.11[b];
  [a][b]amix=inputs=2:normalize=0
    ,afade=type=in:start_time=0:duration=0.002:curve=tri
    ,afade=type=out:start_time=0.052:duration=0.008:curve=tri[out]"
finish tap -3

# ---------------------------------------------------------------------------
# 2. tick.wav — stepper tick for an incrementing number, ~35 ms.
#
#    Same anatomy as tap but higher, thinner and drier: white noise through a
#    2.6 kHz bandpass (1.6 kHz wide) instead of a lowpass, capped at 5.2 kHz so
#    it never fizzes, and a 1450 Hz pip with a 4.5 ms decay instead of 9 ms.
#    The noise tail is cut short (fade-out begins at 2 ms) — that shortness is
#    what reads as "dry". As with tap, the pip is kept subordinate to the noise
#    (0.24) so the sound stays a tick rather than a tiny bell.
# ---------------------------------------------------------------------------
synth tick "
  anoisesrc=color=white:amplitude=0.9:duration=0.035:sample_rate=$SR:seed=202
    ,bandpass=frequency=2600:width_type=h:width=1600
    ,lowpass=frequency=5200:poles=2
    ,afade=type=out:start_time=0.002:duration=0.031:curve=exp
    ,volume=1.0[a];
  aevalsrc=exprs='sin(2*PI*1450*t)*(1-exp(-t/0.0008))*exp(-t/0.0045)':sample_rate=$SR:duration=0.035
    ,volume=0.24[b];
  [a][b]amix=inputs=2:normalize=0
    ,afade=type=in:start_time=0:duration=0.002:curve=tri
    ,afade=type=out:start_time=0.029:duration=0.006:curve=tri[out]"
finish tick -3

# ---------------------------------------------------------------------------
# 3. confirm.wav — two-note ascending confirm, ~180 ms. "Set logged."
#
#    A5 (880 Hz) then E6 (1318.5 Hz) — a rising perfect fifth, the most
#    unambiguously positive two-note interval there is.
#    Each note is a soft triangle: fundamental + 11% third harmonic + 4% fifth
#    harmonic. 5 ms attack (gentle, no click), 55 ms exponential decay.
#    Note two is offset 72 ms by adelay, so the pair lands at exactly 180 ms
#    with no reverb or ring-out beyond the natural decay.
# ---------------------------------------------------------------------------
synth confirm "
  aevalsrc=exprs='(sin(2*PI*880*t)+0.11*sin(2*PI*2640*t)+0.04*sin(2*PI*4400*t))*(1-exp(-t/0.005))*exp(-t/0.055)':sample_rate=$SR:duration=0.18
    ,volume=1.0[a];
  aevalsrc=exprs='(sin(2*PI*1318.5*t)+0.11*sin(2*PI*3955.5*t)+0.04*sin(2*PI*6592.5*t))*(1-exp(-t/0.005))*exp(-t/0.055)':sample_rate=$SR:duration=0.108
    ,volume=0.92
    ,adelay=delays=72:all=1[b];
  [a][b]amix=inputs=2:normalize=0
    ,afade=type=in:start_time=0:duration=0.003:curve=tri
    ,afade=type=out:start_time=0.172:duration=0.008:curve=tri[out]"
finish confirm -3

# ---------------------------------------------------------------------------
# 4. rest-start.wav — soft low double-pulse, ~220 ms. "Rest timer running."
#
#    Two muted A3 (220 Hz) pips, the second at 115 ms. Nearly pure sine — only
#    8% second harmonic — and a 700 Hz lowpass on top, so it sits underneath
#    everything else rather than announcing itself. 8 ms attack (deliberately
#    slower than the taps: this is a state change, not a hit), 45 ms decay.
#    Second pip at 0.85 gain so the pair reads as a soft falling pair.
# ---------------------------------------------------------------------------
synth rest-start "
  aevalsrc=exprs='(sin(2*PI*220*t)+0.08*sin(2*PI*440*t))*(1-exp(-t/0.008))*exp(-t/0.045)':sample_rate=$SR:duration=0.115
    ,volume=1.0[a];
  aevalsrc=exprs='(sin(2*PI*220*t)+0.08*sin(2*PI*440*t))*(1-exp(-t/0.008))*exp(-t/0.045)':sample_rate=$SR:duration=0.105
    ,volume=0.85
    ,adelay=delays=115:all=1[b];
  [a][b]amix=inputs=2:normalize=0
    ,lowpass=frequency=700:poles=2
    ,highpass=frequency=40:poles=2
    ,afade=type=in:start_time=0:duration=0.004:curve=tri
    ,afade=type=out:start_time=0.212:duration=0.008:curve=tri[out]"
finish rest-start -3

# ---------------------------------------------------------------------------
# 5. rest-done.wav — three ascending pips, ~320 ms. "Time's up."
#
#    G5 (784) → B5 (988) → D6 (1175): a G-major triad walked upwards, spaced
#    100 ms. Brighter than rest-start (10% third harmonic, no lowpass) and the
#    third pip is held longer (120 ms vs 100 ms, with a 55 ms decay instead of
#    40 ms) so the phrase resolves instead of stopping. The three pips also rise
#    in gain (0.80 / 0.90 / 1.00). Consonant triad + short attack = clearly
#    terminal without the dissonance or repetition that makes an alarm urgent.
# ---------------------------------------------------------------------------
synth rest-done "
  aevalsrc=exprs='(sin(2*PI*784*t)+0.10*sin(2*PI*2352*t))*(1-exp(-t/0.004))*exp(-t/0.040)':sample_rate=$SR:duration=0.10
    ,volume=0.80[a];
  aevalsrc=exprs='(sin(2*PI*988*t)+0.10*sin(2*PI*2964*t))*(1-exp(-t/0.004))*exp(-t/0.040)':sample_rate=$SR:duration=0.10
    ,volume=0.90
    ,adelay=delays=100:all=1[b];
  aevalsrc=exprs='(sin(2*PI*1175*t)+0.10*sin(2*PI*3525*t))*(1-exp(-t/0.004))*exp(-t/0.055)':sample_rate=$SR:duration=0.12
    ,volume=1.0
    ,adelay=delays=200:all=1[c];
  [a][b][c]amix=inputs=3:normalize=0
    ,afade=type=in:start_time=0:duration=0.003:curve=tri
    ,afade=type=out:start_time=0.312:duration=0.008:curve=tri[out]"
finish rest-done -3

# ---------------------------------------------------------------------------
# 6. impact.wav — soft low-frequency body hit for a hard cut, ~300 ms. A THUD.
#
#    [a] the sub. A sine whose frequency falls exponentially from 115 Hz to
#        55 Hz with a 35 ms time constant — the classic drop that makes a hit
#        read as weight rather than as a tone. The phase is integrated
#        analytically so the sweep is continuous and click-free:
#            f(t)   = 55 + 60*exp(-t/0.035)
#            phase  = 2*PI*(55*t + 60*0.035*(1 - exp(-t/0.035)))
#        A 22% second harmonic keeps it audible on laptop and phone speakers
#        that cannot reproduce 55 Hz. Amplitude decays with an 85 ms constant.
#    [b] the transient. 14 ms of white noise — fourteen milliseconds, not a
#        sweep — lowpassed twice at 700 Hz and highpassed at 60 Hz, so it is a
#        dull knock on the front of the sub, not air. It is fully decayed
#        before t = 15 ms; there is no sustained noise anywhere in this file.
#    A 22 Hz highpass on the mix removes the DC the fast envelope leaves behind.
# ---------------------------------------------------------------------------
synth impact "
  aevalsrc=exprs='(sin(2*PI*(55*t+60*0.035*(1-exp(-t/0.035))))+0.22*sin(4*PI*(55*t+60*0.035*(1-exp(-t/0.035)))))*exp(-t/0.085)':sample_rate=$SR:duration=0.30
    ,volume=1.0[a];
  anoisesrc=color=white:amplitude=0.9:duration=0.014:sample_rate=$SR:seed=303
    ,highpass=frequency=60:poles=2
    ,lowpass=frequency=700:poles=2
    ,lowpass=frequency=700:poles=2
    ,afade=type=out:start_time=0.001:duration=0.013:curve=exp
    ,volume=0.30[b];
  [a][b]amix=inputs=2:normalize=0
    ,highpass=frequency=22:poles=2
    ,afade=type=in:start_time=0:duration=0.002:curve=tri
    ,afade=type=out:start_time=0.288:duration=0.012:curve=tri[out]"
finish impact -3

# ---------------------------------------------------------------------------
# 7. count.wav — tiny high tick for a rapidly counting number, ~20 ms.
#
#    This one gets fired dozens of times per second, so the design goal is
#    "perceptible but never fatiguing": white noise bandpassed at 3.2 kHz and
#    then lowpassed at 6.5 kHz to strip the fizz that makes repeated ticks
#    abrasive, a 2100 Hz pip with a 3.5 ms decay for a hint of pitch, a 2 ms
#    triangular fade-in so nothing lands as an instantaneous edge, and a level
#    ~6 dB under tap.
# ---------------------------------------------------------------------------
synth count "
  anoisesrc=color=white:amplitude=0.9:duration=0.02:sample_rate=$SR:seed=404
    ,bandpass=frequency=3200:width_type=h:width=2200
    ,lowpass=frequency=6500:poles=2
    ,afade=type=out:start_time=0.0015:duration=0.0175:curve=exp
    ,volume=1.0[a];
  aevalsrc=exprs='sin(2*PI*2100*t)*(1-exp(-t/0.0006))*exp(-t/0.0035)':sample_rate=$SR:duration=0.02
    ,volume=0.38[b];
  [a][b]amix=inputs=2:normalize=0
    ,afade=type=in:start_time=0:duration=0.002:curve=tri
    ,afade=type=out:start_time=0.016:duration=0.004:curve=tri[out]"
finish count -3


# ---------------------------------------------------------------------------
# 8. swoosh.wav — notification banner flying across and tucking into the bell,
#    ~260 ms. Explicitly NOT the stock filtered-noise whoosh.
#
#    The generic whoosh is a noise sweep: broadband, unpitched, and it DISPERSES
#    (peaks early, fades out). Everything here is built to be the opposite.
#
#    [a] the body is a TONE, not air — a triangle-ish voice (fundamental + 16%
#        third + 4% fifth) gliding exponentially from C6 (1046.5 Hz) down to
#        C4 (261.6 Hz) with a 40 ms time constant, phase integrated analytically:
#            f(t)   = 261.6 + 784.9*exp(-t/0.040)
#            phase  = 2*PI*(261.6*t + 31.396*(1 - exp(-t/0.040)))
#        The endpoints are deliberate: it starts on the exact pitch ding.wav
#        rings at and falls two octaves, so the pair reads as one gesture —
#        the banner leaves the bell's note, flies down, and the ding answers it
#        back at the top. Downward, not upward: the action is something tucking
#        away and settling, and a rising glide would both contradict that and
#        collide with the ding's register.
#    [b] the amplitude envelope ARRIVES. A fast attack and decay (motion), then
#        a gaussian swell centred at 195 ms — the tuck. Measured per 20 ms the
#        shape is -4 -4 -5 -7 -9 -10 -11 -10 -8 -7 -7 -10 -13 dB: it decays,
#        then gathers back up into the landing instead of trailing off. That
#        late re-concentration is what a stock whoosh never does.
#    [c] a hint of movement, not a wash: white noise through a DOUBLE bandpass
#        at 2.2 kHz (1.4 kHz wide), so it is a narrow band of texture rather
#        than broadband air, and it is faded out over 150 ms — gone before the
#        arrival, leaving the landing pure tone. Gain is set so this layer is
#        ~8% of total energy (measured; the brief's ceiling was 12%).
# ---------------------------------------------------------------------------
synth swoosh "
  aevalsrc=exprs='(sin(2*PI*(261.6*t+31.396*(1-exp(-t/0.040))))+0.16*sin(6*PI*(261.6*t+31.396*(1-exp(-t/0.040))))+0.04*sin(10*PI*(261.6*t+31.396*(1-exp(-t/0.040)))))*(1-exp(-t/0.010))*((0.16+0.84*exp(-t/0.070))+0.30*exp(-((t-0.195)/0.035)*((t-0.195)/0.035)))':sample_rate=$SR:duration=0.26
    ,volume=1.0[a];
  anoisesrc=color=white:amplitude=0.9:duration=0.26:sample_rate=$SR:seed=505
    ,bandpass=frequency=2200:width_type=h:width=1400
    ,bandpass=frequency=2200:width_type=h:width=1400
    ,afade=type=out:start_time=0.004:duration=0.150:curve=exp
    ,volume=4.38[b];
  [a][b]amix=inputs=2:normalize=0
    ,highpass=frequency=60:poles=2
    ,afade=type=in:start_time=0:duration=0.004:curve=tri
    ,afade=type=out:start_time=0.250:duration=0.010:curve=tri[out]"
finish swoosh -3

# ---------------------------------------------------------------------------
# 9. ding.wav — the arrival that answers the swoosh, ~450 ms. Struck metal bar.
#
#    A microwave beep is a square-ish tone with harmonic partials. A struck
#    metal bar is not harmonic at all: an ideal free-free bar rings at
#    1 : 2.756 : 5.404, and it is precisely that INHARMONIC spacing that the ear
#    hears as "metal" rather than "flute". Those exact ratios are used here, on
#    a C6 (1046.5 Hz) fundamental — the bottom of the suggested range, chosen
#    because it is warm rather than shrill, and because the swoosh glides down
#    from this same pitch.
#      partial 1: 1046.5 Hz   gain 1.00   decay 130 ms
#      partial 2: 2884.2 Hz   gain 0.22   decay  45 ms
#      partial 3: 5655.3 Hz   gain 0.06   decay  18 ms
#    The upper partials decay much faster than the fundamental, which is both
#    what real metal does and what keeps this warm: the strike is bright for a
#    few tens of ms, then the sound mellows into a clean fundamental instead of
#    ringing shrill for half a second.
#    The fundamental is a doublet — 1046.5 Hz plus 1048.5 Hz at 0.45 gain. Real
#    bells have slightly detuned mode pairs; the resulting 2 Hz beat is under
#    one full cycle across the file, so it reads as liveliness, not wobble.
#    Pure exponential decay to -30 dB by 450 ms, then a 12 ms close. No reverb.
# ---------------------------------------------------------------------------
synth ding "
  aevalsrc=exprs='(sin(2*PI*1046.5*t)+0.45*sin(2*PI*1048.5*t))*(1-exp(-t/0.0015))*exp(-t/0.130)':sample_rate=$SR:duration=0.45
    ,volume=1.0[a];
  aevalsrc=exprs='sin(2*PI*2884.2*t)*(1-exp(-t/0.0010))*exp(-t/0.045)':sample_rate=$SR:duration=0.45
    ,volume=0.22[b];
  aevalsrc=exprs='sin(2*PI*5655.3*t)*(1-exp(-t/0.0008))*exp(-t/0.018)':sample_rate=$SR:duration=0.45
    ,volume=0.06[c];
  [a][b][c]amix=inputs=3:normalize=0
    ,afade=type=in:start_time=0:duration=0.002:curve=tri
    ,afade=type=out:start_time=0.438:duration=0.012:curve=tri[out]"
finish ding -3

# ---------------------------------------------------------------------------
# 10. pop.wav — a card landing on screen, ~90 ms. A round low-mid "pock".
#
#     Same family as tap but a different object: tap is a fingertip on glass
#     (bright, dry, noise-led, 60 ms), pop is a small solid thing settling
#     (round, pitched, body-led, 90 ms). The difference is carried by making
#     the TONE dominant here rather than the noise, and putting it an octave
#     below tap's 560 Hz pip.
#     A sine drops from 320 Hz to 175 Hz with a 20 ms time constant — the same
#     analytic phase integration as impact.wav, just smaller and faster — with
#     15% second harmonic for body on phone speakers. 2 ms attack, 28 ms decay.
#     The front transient is only 8 ms of noise, lowpassed TWICE at 900 Hz and
#     highpassed at 120 Hz: enough to mark the contact, far too dark to click.
# ---------------------------------------------------------------------------
synth pop "
  aevalsrc=exprs='(sin(2*PI*(175*t+2.90*(1-exp(-t/0.020))))+0.15*sin(4*PI*(175*t+2.90*(1-exp(-t/0.020)))))*(1-exp(-t/0.002))*exp(-t/0.028)':sample_rate=$SR:duration=0.09
    ,volume=1.0[a];
  anoisesrc=color=white:amplitude=0.9:duration=0.008:sample_rate=$SR:seed=606
    ,highpass=frequency=120:poles=2
    ,lowpass=frequency=900:poles=2
    ,lowpass=frequency=900:poles=2
    ,afade=type=out:start_time=0.0008:duration=0.0072:curve=exp
    ,volume=0.18[b];
  [a][b]amix=inputs=2:normalize=0
    ,highpass=frequency=35:poles=2
    ,afade=type=in:start_time=0:duration=0.002:curve=tri
    ,afade=type=out:start_time=0.082:duration=0.008:curve=tri[out]"
finish pop -3

echo "make-sfx: done — 10 files in $OUT_DIR"
