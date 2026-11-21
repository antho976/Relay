import React from 'react';
import {Audio, Sequence, staticFile} from 'remotion';

/**
 * The brief: sound on the things that would actually make a sound — a tap, the watch, a timer — and
 * nothing under the transitions. Every cue is an interaction the viewer can see happen on screen,
 * and every cue means one thing:
 *
 *   tap      a page or a list row landing
 *   pop      a card settling (the arrival banner, the nine new onboarding pages)
 *   impact   the one hit that clears the old onboarding
 *   tick     the watch stepper
 *   confirm  a set being logged
 *   swoosh   the banner flying into the bell
 *   ding     the bell
 *   sweep    the version changing under the accent edge, and the Home morph
 *   reveal   one element arriving: a view switching, the fifth tab, the closing line
 *
 * Three cues from the first cut are retired, not re-levelled. `screen` fired on nearly every beat,
 * often eight frames into a push where nothing on screen had changed, and repeated that often it
 * read as a noise with no referent. `fill` was a glass-xylophone run laid under the Home goal bars
 * while the bed was still silent, which made it the loudest thing in the first ten seconds of the
 * film; a meter filling does not need to be scored. `count` was a 20 ms tick meant for a rapidly
 * counting number and was being used for a list landing, which `tap` already means.
 *
 * Synthesised by tools/make-sfx.sh; nothing here is licensed material.
 */
export const SFX = {
  tap:       'sfx/tap.wav',
  tick:      'sfx/tick.wav',
  confirm:   'sfx/confirm.wav',
  restStart: 'sfx/rest-start.wav',
  restDone:  'sfx/rest-done.wav',
  impact:    'sfx/impact.wav',
  swoosh:    'sfx/swoosh.wav',
  ding:      'sfx/ding.wav',
  pop:       'sfx/pop.wav',
  reveal:    'sfx/reveal.wav',
  sweep:     'sfx/sweep.wav',
} as const;

export type SfxName = keyof typeof SFX;

/** A cue placed by a beat: the frame it fires on (beat-local) and what it is. */
export type CuePoint = {at: number; sfx: SfxName; gain?: number};

/**
 * The one place the cues are balanced against each other. `tools/make-sfx.sh` normalises every
 * file to the same -3 dBFS peak precisely so this map is the only thing shaping the hierarchy —
 * setting it in both places attenuated everything twice and put the film's loudest moment at
 * -14.5 dBFS, which survives headphones and nothing else.
 *
 * The transients sit high on purpose. They carry a 22-24 dB peak-to-loudness crest against the
 * tonal cues' 6-9, so a tap at 0.8 and a chime at 0.62 are nowhere near equally loud to a listener
 * — matching them by peak is what makes them read as one palette.
 */
const LEVEL: Record<SfxName, number> = {
  // impact at 0.9 summed with the bed's own downbeat kick — the hit now lands on one — peaked
  // the finished mix at -1.35 dBFS; 0.72 holds the delivery ceiling of -2 with the hit still the
  // loudest event in the film.
  tap: 0.8, tick: 0.62, confirm: 0.5, restStart: 0.5, restDone: 0.6, impact: 0.72,
  swoosh: 0.62, ding: 0.7, pop: 0.75, reveal: 0.6, sweep: 0.55,
};

/** Master trim — one place to pull the whole sound design up or down against picture. */
export const MIX = 0.89;

/** Fire a sound at a frame within the current scene. */
export const Cue: React.FC<{at: number; sfx: SfxName; gain?: number}> = ({at, sfx, gain = 1}) => (
  <Sequence from={at} durationInFrames={90} layout="none" name={`sfx:${sfx}`}>
    <Audio src={staticFile(SFX[sfx])} volume={LEVEL[sfx] * gain * MIX} />
  </Sequence>
);

/**
 * How many frames before its cue an entrance spring has to start so the thing is visibly *there*
 * when the sound says it landed. A Remotion spring at stiffness 140-210 (damping 200) is at 0.6-0.7
 * five frames in and 0.8-0.9 at eight; starting it on the cue frame meant every tap sounded a
 * sixth of a second before the page it announced had appeared.
 */
export const LEAD = 5;
