# Avex 0.9 — release film

> **Building another one? Read [RULES.md](RULES.md) first.** It is the list of mistakes this cut
> already made, with the measurements that caught each one. Every rule in it cost a round.

90 seconds, 1920×1080, 30fps. `npm run studio` to work on it, `npm run render` to output
`out/avex-0.9.mp4`, `npm run render:4k` for the 3840×2160 delivery (CRF 18, ~120 MB, under Discord's 500 MB
Nitro cap — there is no point going past 4K: the captures are 1080×2400 and are already shown
upscaled 1.5× at 4K, so a larger frame adds pixels to the footage without adding detail). `npm run check` prints every handover against its bar and fails if one is off the grid.
Chromium is not downloaded on this machine: pass `--browser-executable=/usr/bin/chromium` to the
render commands (or set it once in a `remotion.config.ts`).

**The picture and the sound are rendered separately and muxed by `tools/deliver.sh`.** Remotion's
own mp4 carries 2048 samples of AAC encoder priming that the container does not declare, so every
player starts the sound 42.7 ms (1.3 frames) after the picture — measured by cross-correlating the
rendered file against the bed (0.993 correlation at +42.3 ms) and against `impact.wav` at the hit.
A `--codec=wav` render of the same composition has no lag at all (0.00 ms, the impact 0.3 ms from
its frame), so that is the soundtrack, and ffmpeg's AAC encoder writes the edit list that players
honour. Measure the delivered file, not the render.

## What the edit is built on

**Pace.** Sixteen beats in ninety seconds cannot all be the same length; the 0.8.9 cut ran 3:36
because every beat was ~11 seconds and entered the same way. A beat's length is declared by the bar
it ends on, and its speed decides how much copy it is allowed to hold — a paragraph at 6s, one line
at 4s, a headline alone at 3s, cut hard on both sides. `XFADE.cut` is a genuine zero-frame cut: an
edit where all fifteen joins are crossfades has no punctuation, only commas. No beat sits on a still
capture with no camera: where a capture is static, the beat is timed so the capture's own action (a
set logging, a toggle, a scroll) happens inside it, on the grid.

**One device.** Before and after are the same phone, never two. A side-by-side halves both screens
and turns a release into spot-the-difference. `Seam` (in `Device.tsx`) stacks the two captures in one
body and sweeps an accent edge between them — AccentRed replacing AccentNavy being, literally, what
0.9 shipped. The same gesture does the section breaks, as the `sweep` transition.

**Two kinds of camera, and always one running.** `Device.focus` flies a window *inside* a capture;
`Camera`/`Shot` moves the whole frame. They are not interchangeable: `focus` at z > 1 crops
horizontally as well as vertically, so it eats the edges of words. In-screen travel therefore belongs
to `Detail`, which fills the frame edge to edge; a `Solo` shows the whole phone and moves the camera
on it instead. Every beat carries one or the other, and the joins are `push` — both frames travelling
together, so the camera seems to pan from one subject to the next.

The accent edge stays *inside* the phone, where it is one device changing version. Blown up to the
whole frame as a transition it stopped being that and became a red bar crossing the screen, so it is
gone as a cut.

**Onboarding is drawn as phones, not cards.** The first version used landscape wireframe tiles and
read as a Figma export dropped into a film made of real screens. The fix was shape and chrome, not
size: a portrait body with a status bar, a step rail and a real CTA reads as the product at any
scale. Its three era tells are the vanishing chapter eyebrow, the continuous step bar becoming
segmented, and 0.8.9's white pill CTA becoming 0.9's accent slab.

**Drawn and filmed, in one body.** `Device` takes `children`, so a natively rebuilt Compose screen
(`ui/`) and a real capture sit in the same phone. Some beats can only be drawn — the Home morph needs
both eras on one clock, the watch is a 454px round display that no recording makes legible at 1080p,
and the notifications beat turns on a banner flying into a bell that a screen recording renders as
four frames of something small moving.

## Sound

`public/sfx/` — cues synthesised or finished by `tools/make-sfx.sh`, nothing licensed. The watch
detent (`tick`) is synthesised now; the keyboard-switch take it replaced kept 53% of its energy
above 4 kHz and read as hiss on five repeats. Sound only
where the picture shows an interaction that would make one, and every cue means exactly one thing:
`tap` a page or a row landing, `pop` a card settling, `impact` the hit, `tick` the watch stepper,
`confirm` a set logged, `swoosh`/`ding` the banner into the bell, `sweep` the version changing (every
seam, and the Home morph), `reveal` one element arriving (a view switching, the fifth tab, the
closing line). The music bed is `public/music/bed.mp3`, ridden under the picture in `Release.tsx`.

Three cues from the first cut are retired rather than re-levelled. `screen` fired on nearly every
beat — after every seam, on the Home turn, and eight frames into every Solo, inside the push where
nothing on screen had changed — and at that rate a "muted card set down" is a noise with no
referent. `fill`, a glass-xylophone run, was laid under the Home goal bars filling while the bed was
still silent, which made it the loudest thing in the first ten seconds of the film; a meter filling
does not make a noise. `count` was a 20 ms tick meant for a rapidly counting number and was carrying
the list, which `tap` already means.

**Where a cue accompanies an entrance, the entrance leads the cue by `LEAD` frames** (five). A
Remotion spring at the stiffnesses used here is at 0.6–0.7 five frames in; starting it on the cue
frame meant every tap sounded a sixth of a second before the page it announced had appeared. The
watch is the other way round on purpose: the tick frames drive the number, so they are one instant.

`swoosh.wav` is pitched, not a noise sweep — a C6→C4 glide that re-gathers energy before it lands,
tuned so it *starts* on the exact pitch `ding.wav` rings at. A stock whoosh only ever disperses.

`make-sfx.sh` normalises every file to the same **−3 dBFS** peak, so `Sound.tsx`'s `LEVEL` map
is the only thing shaping the hierarchy. Setting it in both places attenuated every cue twice and put
the film's loudest moment at −14.5 dBFS — which survives headphones and nothing else.

The transients sit high in that map on purpose: they carry a 22–24 dB peak-to-loudness crest against
the tonal cues' 6–9, so a tap at 0.8 and a chime at 0.62 are nowhere near equally loud to a listener.
Matching them by peak is what makes them read as one palette. Integrated loudness for the film is
low by design — most of it is silence — so judge the mix by cue peaks, not by LUFS.

## Footage

`public/cfr/` — every clip transcoded to **constant** 30fps. `adb screenrecord` writes variable frame
rate (these takes averaged ~56fps), so a frame index measured with ffmpeg meant a different instant
to Remotion, which counts composition frames at 30. The offsets in `Release.tsx` only land where they
were measured because the two now agree.

Unused but available: `before/after-settings`, `before/after-stats`, `before-home`, `after-checkin`.

## Claims

Every figure on screen is either visible in the footage or asserted by the source:

| On screen | Source |
|---|---|
| 147 commits · 632 files | `git log/diff --stat 2fe2379..main` |
| 15 pages down to 9 | 0.8.9 `PAGE_WELCOME`(0)…`PAGE_PREVIEW`(14); 0.9 `GENERATED_PATH` has 9 |
| Thirty-five pieces, five tracks | `docs/ACADEMY_ART.md`; the app's own masthead reads `35 PIECES · 40 MIN` |
| volume no longer truncates to 52… | `before-coach.mp4` shows the header reading `52....` |
| the twelve-row year grid | `before-lasttab.mp4` shows `THIS YEAR` as JAN–DEC dot rows |

An earlier cut of this film claimed **145 commits · 460 files**, **28 Academy lessons** and
**seventeen pages down to ten**. None of the three survives contact with the code — the lesson count
comes from a commit message that its own commit contradicts (31 lessons + 4 articles = 35 pieces),
and no tree has ever produced 17 or 10 onboarding pages.

**Open:** `versionName` is still `0.8.8.3` in `forge-android/app/build.gradle.kts` and `versionCode`
is 89. Nothing in `2fe2379..main` bumps either. The film says 0.9 throughout; the build should be
bumped before it ships, or the film is announcing a version the app does not report.
