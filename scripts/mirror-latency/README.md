# Mirror latency

Measures the device mirror end to end — from the moment the phone draws a frame to the moment
the desktop compositor presents it, and from a tap sent to the photon that answers it — and
splits that into stages: device (capture, encode, adb), engine, socket, client parse, decode,
GTK, compositor. Use it to A/B a setting (bit rate, `max_fps`, codec options, capture size) or a
build of Relay against your own phone.

What is here:

| file | what |
|---|---|
| `probe-app/` | **LatProbe**, a one-activity Android app (Java, no Gradle). Its left third is a barcode of the device's `CLOCK_MONOTONIC` at the moment each frame was drawn (24 stripes, 0.1 ms units) under a magenta marker; the right two thirds flip black/white on every touch. A local socket (`latprobe`) answers clock-sync pings and takes commands: animate or stand still, light or heavy content, a color chart. `build.sh` builds the APK with the SDK's command-line tools. |
| `probe.py` | The driver: starts the probe, syncs the device's clock with the host's, lets it animate, then taps the field through the engine (animating, then on a still screen), and writes its own trace lines. |
| `analyze.py` | Joins the engine's, the client's and the driver's trace lines on (mirror, packet seq) and prints each stage's median, p95 and max, plus tap-to-photon. |
| `summary.py` | One line of medians per run, for comparing runs side by side. |
| `check-colors.py` | Reads the probe's color chart back from a screenshot of the mirror and compares it with what the device drew, and with another screenshot. |

Nothing here is built into Relay. The trace points are: `RELAY_MIRROR_TRACE=1` turns them on in
both the engine and the native client (one load per packet when off), and each process logs them
at the `mirror_trace` tracing target — the format is in `docs/engine/BUS.md` (*Mirror trace*) and
`apps/relay-native/src/mirror/trace.rs`.

## Running it against your phone

1. Build and install the probe (Android SDK with build tools and a platform; `ANDROID_HOME`
   defaults to `~/Android/Sdk`):

   ```fish
   scripts/mirror-latency/probe-app/build.sh            # → target/mirror-latency/probe/latprobe.apk
   adb -s SERIAL install -r target/mirror-latency/probe/latprobe.apk
   ```

2. Start the engine and the window with tracing on, each logging to a file. Quit any running
   `dev` engine first (`relay --instance dev q app.quit '{}'`) — the engine reads the variable
   when it starts:

   ```fish
   set -x RELAY_MIRROR_TRACE 1
   mkdir -p ~/mlat/run1
   RELAY_LOG=info,mirror_trace=info target/debug/relay --instance dev serve > ~/mlat/run1/engine.log 2>&1 &
   RUST_LOG=mirror_trace=info target/debug/relay-native > ~/mlat/run1/native.log &
   ```

3. Open the phone's mirror from Relay's Devices panel and keep it visible (docked or in its window):
   a hidden window paints nothing, and the stages after decode are then not measured.

4. Drive the probe (on the phone it takes the screen; touch nothing while it runs):

   ```fish
   scripts/mirror-latency/probe.py --device SERIAL --native-log ~/mlat/run1/native.log --out ~/mlat/run1/driver.log
   ```

   Options: `--anim 10` seconds of animation first, `--taps 25` per phase, `--gap 0.7` seconds
   between taps, `--heavy` (1500 random squares a frame: an encoder working hard), `--chart 10`
   (end on the color chart for ten seconds; take a screenshot of the window meanwhile for
   `check-colors.py`). `--socket` or `--instance` if the engine is not the `dev` one.

5. Read it:

   ```fish
   scripts/mirror-latency/analyze.py ~/mlat/run1/{engine,native,driver}.log
   scripts/mirror-latency/summary.py ~/mlat/run*      # one line per run
   ```

To A/B a setting, change it (Settings, or `relay q settings.set …` for `device.mirror.max_fps`,
`device.mirror.video_codec_options`, `device.mirror.pin_refresh_rate`), reopen the mirror, and run
step 4 again into a new directory. Alternate A and B two or three times each: one run is noisy,
and a busy host (a build, an emulator) moves every number.

## Reading the numbers

- **device: draw → engine has the header** is everything on the phone and the cable: the app's
  frame reaching the compositor, capture, the hardware encoder and adb. It is most of the total
  and it is where encoder settings (`max_fps`, bit rate, vendor low-latency keys) show up.
- **decode**, **gtk: texture built and set** and **gtk: paint** are the client's own costs.
- **gtk: set → frame painted** and **compositor: painted → presented** wait for the display's
  next frame; they move with the monitor's refresh rate, not with Relay.
- **tap → presented** on a still screen is the latency a person feels: the touch's trip to the
  device, the app's reaction, and the whole video path back.

The device's clock is synced with the host's twice per run (before and after); `analyze.py`
uses their mean. Over a minute the two agree to well under a millisecond.

## What the in-process decoder changed (2026-10)

The client used to pipe every packet through an `ffmpeg` child and read RGB pictures (PPM) back;
it now decodes with libavcodec on a thread of its own (low delay, one thread, one access unit
per packet, no parser) and gives GTK the YUV 4:2:0 planes, converted on the GPU. Measured with
this probe on an emulator (Tally_API36, 576×1280 at 60 fps, the same engine for both builds,
three runs each, medians of medians):

| median, ms (CPU in %)                      | light, before | light, after | heavy, before | heavy, after |
|--------------------------------------------|--------------:|-------------:|--------------:|-------------:|
| decode (packet in → picture in hand)       |          0.54 |         0.18 |          1.24 |         0.88 |
| GTK paint (snapshot + upload + render)     |          0.64 |         0.28 |          0.66 |         0.27 |
| device draw → decoded                      |         23.35 |        22.76 |         24.56 |        24.24 |
| device draw → painted                      |         24.38 |        24.05 |         25.41 |        24.92 |
| device draw → presented                    |         45.11 |        45.41 |         44.52 |        46.02 |
| tap on a still screen → presented          |         37.21 |        36.87 |         38.21 |        36.48 |
| client CPU (window + `ffmpeg` child)       |          22.0 |          2.3 |          25.6 |          5.2 |

"Light" is the probe alone (packets ~1 KB); "heavy" adds 1500 random squares a frame (~26 KB).
Before, "decode" ran from the write into the child's stdin to the last byte of its PPM read
back. The client's own share of a frame went from ~1.2–1.9 ms to ~0.5–1.2 ms and its CPU by a
factor of five to ten; the end-to-end figures barely move, because they are dominated by the
device (≈23 ms from draw to the engine on this emulator) and by the compositor's frame pacing
(the headless KWin used here holds a painted frame ~20 ms), neither of which the client sets.
Colors read back from the chart differ between the two builds by at most 4 levels of 255 on a
saturated patch and not at all on grays (`check-colors.py`): the decoder honors the stream's
VUI (the emulator tags BT.709, narrow range) as swscale did.
