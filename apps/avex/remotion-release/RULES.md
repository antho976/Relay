# Rules for building an Avex release film

Point at this file at the start of the next one.

Everything here was learned by getting it wrong on the 0.9 cut. Each rule carries the evidence, so
none of it has to be taken on trust — and so a future disagreement can be settled by re-measuring
rather than by re-arguing.

---

## 1. Claims

**Trace every on-screen number to code or footage before animating it.** Not to a commit message, not
to a doc, not to the previous cut.

The 0.8.9 prototype put three false claims on screen:

| Claimed | Actual | Where the truth was |
|---|---|---|
| 17 onboarding pages down to 10 | **15 → 9** | the page dispatch in `OnboardingScreen.kt`, both refs |
| 28 Academy lessons | **35 pieces** (31 lessons + 4 articles) | `AcademyRegistry`, and `docs/ACADEMY_ART.md` |
| 145 commits · 460 files | **147 · 632** | `git log/diff --stat 2fe2379..main` |

The lesson count came from a commit message (`139e473`, *"Rewrite all 28 Academy lessons"*) that its
own commit contradicts. **A commit message is a claim, not a source.**

**When code and a comment disagree, the code wins.** `ForgeBottomBar`'s comment says the Academy "had
been a link inside Coach". `ui/academy/` does not exist at 0.8.9 at all — the comment describes a
mid-0.9 state. That comment put a false sentence on screen for two rounds.

**Check the footage too.** A note claimed 0.8.9's goal meters "filled neutral". `before-home.mp4`
shows them filled navy. What changed was which colour the accent is.

---

## 2. Copy

**No em dashes.** They are the single strongest tell that a machine wrote the line. Same for
tricolons ("no levels, no unlocking, no progress bar"), for "not X but Y", and for a headline that
scans as clever and lands as nothing.

Two real examples of the failure and its fix:

- *"It shows its working"* → **"Coach explains itself"** / **"Coach shows its work"**
- *"The coach, not a footnote to it"* → **"Learn why you're doing it"**

The second was worse than opaque, it was **false**: the Academy is a place to read, not the coach.
Clever phrasing hides wrong phrasing.

**Say what the thing does, in the words a person would say out loud.** If the line would sound strange
spoken to a friend, rewrite it.

**Watch the small words.** "Academy gets **its own** tab" implies it had one before. It did not. It
gets **a** tab.

---

## 3. Timing, once there is music

**Never declare a beat in seconds.** Declare it by **the bar it ends on**. A beat length in seconds
survives exactly until someone adds a soundtrack, and then every cut in the film is subtly wrong.

**Measure the tempo off the rendered audio. Never trust the prompt — and do not trust one
measurement either.** The bed was asked for at 120 BPM. The first measurement said **120.19** with
the first downbeat at 0.116 s and the whole cut was laid on that. A second one, done because the
director said cuts were off the beat, says **120.021** with the first beat at 0.008 s, residual
2.2 ms rms over 110 beats. The first grid put every cut in the first half of the film **two to three
frames late** and only agreed with the music around bar 33 — which is why the one cut everyone
checked (the tab swap) sounded right and the rest did not.

The measurement chain that works, all in numpy, no dependencies:

```
spectral-flux onset envelope at a ~1.5 ms hop
  → autocorrelation for a first guess at the period
  → the onset peak nearest each predicted beat
  → least-squares line through those peaks: period and phase, with a residual you can quote
  → downbeat = whichever of the 4 phases carries the most low-band flux
```

A coarse-hop autocorrelation alone gives you the period to ±0.2 BPM and the phase to ±100 ms. The
regression is what gets both to the frame. If the residual is not a few milliseconds, the track is
not at a constant tempo and no grid will fit it.

**Derive lengths so the handover lands on the downbeat, in whole frames.** A transition of T frames
overlaps the two sequences by T, so it has to straddle the bar: the next beat starts floor(T/2)
before the downbeat and this one runs ceil(T/2) past it. Splitting T in half as a float put every
beat after a seven-frame whip on a half frame, and every cue placed by `bar(n) − start` was then
rounded off the grid by Remotion.

```
start[i] = bar(endBar[i−1]) − floor(T[i−1]/2)
len[i]   = bar(endBar[i]) + ceil(T[i]/2) − start[i]
```

Verify it: `npm run check` prints every handover and fails if one is off. If that check ever fails,
the arithmetic is wrong, not the music.

**Changing one beat's length shifts every downbeat after it.** "Make the watch two seconds longer"
means re-deciding the bar allocation, not bumping a number. Cheap, but say what it costs the
neighbours.

**Sync the film's biggest moment to the track's.** Find the bar where the bed drops out and returns,
and put the loudest visual event on it. Worth more than any other single timing decision.

---

## 4. Sound

**Quantise every cue to the grid.** This is the whole difference between a sound design and a pile of
noises, and it was the note that took longest to understand. Cues that fire on whatever frame their
animation happens to reach sound, in the director's words, "all over the place".

**Where a cue accompanies an animation, the animation follows the cue.** Not the reverse. The watch
weight changes *on* the tick because the tick frames drive the number, not because they were tuned to
line up.

**Where a cue accompanies an entrance, start the entrance before the cue.** A spring at the
stiffnesses used here is at 0.6–0.7 five frames in. The first cut started every page's spring on the
frame its tap fired, so every tap sounded a sixth of a second before the page it announced existed.
`LEAD` in `Sound.tsx` is that offset, and every entrance that has a sound uses it.

**A cue has to have a referent, every time it fires.** `screen` fired eight frames into every Solo
beat because "the capture visibly changes screen" was written into the prop name and nobody checked
that it did; eight frames in is inside the push transition, where the only thing changing is the
transition. Repeated nine times, a sound with no referent reads as a noise, and it was the second
note the director gave. Before placing a cue, name the frame on the capture that makes the noise.

**Do not score things that do not make a noise.** A meter filling, a number counting, a colour
changing. The glass-xylophone run under the Home goal bars was the first note the director gave, and
it was also the loudest thing in the first ten seconds because the bed had not entered yet.

**Lay runs out in float, round per event.** At 120 BPM a sixteenth is 3.74 frames. Rounding the step
to 4 and multiplying drifts a sixteenth of a beat every four events.

**Normalise every sound file to one peak. Put the hierarchy in exactly one place.** Setting it in both
the files and the level map attenuated everything twice and put the film's loudest moment at
−14.5 dBFS, which survives headphones and nothing else.

**Transients need more gain than tones to sound equally loud.** A tap carries a 22–24 dB
peak-to-loudness crest against a chime's 6–9. Matched by peak, a tap at 0.8 and a chime at 0.62 are
nowhere near equal to a listener.

**Bright plus sustained equals a tool running.** A cue was rejected as "an electric cutter cutting
metal". Its measurements had predicted exactly that and were ignored: **24% of energy above 4 kHz
with a 320 ms sustained peak**. The replacement passes hard limits — 0.0% above 4 kHz, peak held for
one 20 ms window.

Set numeric limits before generating, and reject on the numbers:

| | |
|---|---|
| soft / warm cue | < 5% of energy above 4 kHz, ≥ 55% below 1 kHz |
| any cue | peak must not hold for more than ~120 ms |
| everything | first sample frame exactly `0 0`, or it clicks |

**Prompt the strike, not the adjective.** "A soft deep thud" returned 100% of its energy below 200 Hz
with no attack at all — a formless sub. Describe the object and the impact.

**Silence is a level.** The film's sound should have an arc the way the music does. The current one
runs −71 (silence) → −33 → −24 → −21 → −19 at the peak → −26 to close. Flat density reads as noise.

**Measure the delivered file against the picture, not the render against itself.** Remotion's mp4
put the whole soundtrack 42.7 ms (2048 samples of undeclared AAC priming) behind the picture, in
this cut and the one before it. Every cue was on its frame in the composition and 1.3 frames late in
the file. Cross-correlate the delivered audio against the bed and against one sample at a known
frame; `tools/deliver.sh` exists because of this.

**Measure the finished mix, not the automation.** The first fader ride on the bed overshot and put the
climax **1.7 dB below** the middle of the film — worse than the problem it was fixing. It only showed
up in a per-second RMS pass over the rendered file.

Delivery targets: **−18 LUFS integrated, peak no hotter than −2 dBFS.** Anything above −1 risks true
peak overshoot once it is through a lossy encode.

---

## 5. Motion

**Two states sharing one slot must hand over, never crossfade.** This bug appeared **three separate
times** — the era tag rendered `00899`, the Home morph double-exposed two different sentences, and
the tab bar showed "Profile" and "Academy" on top of each other. At 50/50 both are legible and
neither is readable. The old one leaves before the new one arrives:

```ts
const out = clamp((0.44 - t) / 0.44);
const inn = clamp((t - 0.56) / 0.44);
```

**Grain must be regenerated per frame or be static. Never translated.** Sliding a 220 px tile 37 px
across and 53 px down each frame wraps every 6 and 4 frames — a 5–7 Hz strobe over the whole picture.
It was reported as "the assets and text seem to be bouncing up and down", and it took a while to find
because nothing in the animation code was bouncing.

**Never crop the subject to the frame edge.** Full-bleed crops of a capture put copy on top of live UI
text and slice headlines in half. A phone that stays a phone with the copy beside it is worth more
than the extra legibility.

**Zooming inside a screen crops horizontally too.** `focus` at z > 1 ate the edges of words —
"History" rendered as "ory", "150" as "50". In-screen travel belongs to a full-frame treatment; a
device shot moves the camera on the device instead.

**Every beat needs a camera.** A held frame in a ninety-second film reads as a slide.

**A cut is one frame, so nothing may fade across it.** Every beat used to fade itself in and out
over ten frames, which turned each hard cut into a dip to black and each seven-frame whip into a
throw between two frames at 35% brightness. On a hard cut the picture was darkest ON the downbeat
and the new frame arrived a third of a second later — which is exactly what "the cuts are off the
beat" feels like when the grid is right. `Edges` in `Type.tsx` gives every beat zero fade at a
join; only the film's first frame fades up and its last fades down. Render a strip of frames across
every join and look at the frame ON the bar: it has to be a full picture.

**Check the render, not the code.** The era indicators were `<div ... />` — self-closing, so the
`label` prop was never rendered and both pills drew as empty outlines. It shipped through several
rounds because the code looked correct. Render a still of every beat and actually look at it.

---

## 6. Captures

**Transcode to constant frame rate before anything else.** `adb screenrecord` writes VFR (these takes
averaged ~56fps), so a frame index measured with ffmpeg means a different instant to Remotion, which
counts composition frames at 30. Every offset in the edit depends on this. `tools/capture.sh` does it.

**Profile every capture for motion and seam only on stillness.** A version change that fires while the
old screen is still coasting from a scroll reads as a glitch. Downscale to greyscale, diff consecutive
frames, and find the windows under threshold:

```
ffmpeg -i clip -vf scale=64:142,format=gray -f rawvideo -
```

Then place the seam so it *completes* inside a window where **both** clips are at rest. After the seam
the clip may move as much as it likes.

**Verify the pair before trusting an offset.** Extract the two frames side by side and look. The Coach
seam works because `52....` sits directly beside `52.8k lb` — that was found by looking, not by
assuming.

**Check what a capture does later in its run.** The Coach clip tours down to the block rail and then
**scrolls back up**, so the usable stretch is only ~130 frames and the playback rate has to be set
against it.

---

## 7. Process

**Spawn agents for research and generation, never for taste.** They can mine 147 commits, measure a
spectrum, or run 28 generations. They cannot tell you whether a track sounds good.

**Say plainly what cannot be verified.** Nobody in this loop can hear the audio. Every claim about
sound in this repo is a measurement, and it is stated as one. "It sounds better" is not a finding.

**Ask when a note is ambiguous.** "The close-up needs to go" was read as the blown-up component and
the wrong thing was cut; it meant the full-bleed crops. One clarifying sentence would have saved a
round. Name the beat back before cutting it.

**Report what was traded.** Restoring one beat cost another one's seconds. Say which, every time.

---

## Standing facts about this film

- Bed: `public/music/bed.mp3`, **120.021 BPM**, bar 1.99964 s, first beat 0.0082 s, 46 bars.
  At 30 fps that is 60 frames a bar to within rounding: bar n is frame 60·(n−1).
- Cut: 16 beats, 2700 frames, **90.00 s**, ending exactly on bar 46. Syncs: Home turns red on
  bar 5 (bed enters), watch→Coach on 17 (bed opens), chart on 25 (bed drops back), tab swap on 33
  (bed returns), closing line on 45 (bed drops out).
- Captures are 1080×2400 CFR 30 in `public/cfr/`
- Unused but available: `before/after-settings`, `before/after-stats`, `before-home`, `after-checkin`
- **`versionName` is still `0.8.8.3` and `versionCode` 89 at `main`.** Nothing in `2fe2379..main`
  bumps either, and the film says 0.9 in eight places. Bump the build before it ships.
- 4K: no upscaling involved. The phone renders ~1800 px tall from a 2400 px source, so 4K is sharper
  than 1080p rather than softer. CRF 20 should land well under 400 MB.
