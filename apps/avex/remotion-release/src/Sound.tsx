import React from 'react';
import {Audio, Sequence, staticFile} from 'remotion';

/**
 * The brief: sound on the things that would actually make a sound — a tap, the watch, a timer — and
 * explicitly no whoosh under the transitions. So there is no ambience and no music bed; every cue is
 * an interaction the viewer can see happen on screen. Silence is the default state, not a gap.
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
  count:     'sfx/count.wav',
} as const;

export type SfxName = keyof typeof SFX;

/** House levels, so a beat asks for "a tap" rather than guessing a number. */
/**
 * The one place the cues are balanced against each other. `tools/make-sfx.sh` normalises all seven
 * files to the same -3 dBFS peak precisely so this map is the only thing shaping the hierarchy —
 * setting it in both places attenuated everything twice and put the film's loudest moment at
 * -14.5 dBFS, which survives headphones and nothing else.
 *
 * The transients sit high on purpose. They carry a 22-24 dB peak-to-loudness crest against the
 * tonal cues' 6-9, so a tap at 0.8 and a chime at 0.62 are nowhere near equally loud to a listener
 * — matching them by peak is what makes them read as one palette.
 */
const LEVEL: Record<SfxName, number> = {
  tap: 0.8, tick: 0.62, confirm: 0.62, restStart: 0.5, restDone: 0.6, impact: 0.9, count: 0.3,
};

/** Master trim — one place to pull the whole sound design up or down against picture. */
export const MIX = 0.9;

/** Fire a sound at a frame within the current scene. */
export const Cue: React.FC<{at: number; sfx: SfxName; gain?: number}> = ({at, sfx, gain = 1}) => (
  <Sequence from={at} durationInFrames={90} layout="none" name={`sfx:${sfx}`}>
    <Audio src={staticFile(SFX[sfx])} volume={LEVEL[sfx] * gain * MIX} />
  </Sequence>
);

/** A run of the same cue — the counter ticking, a list landing item by item. */
export const CueRun: React.FC<{
  from: number; every: number; count: number; sfx: SfxName; gain?: number; decay?: number;
}> = ({from, every, count, sfx, gain = 1, decay = 1}) => (
  <>
    {Array.from({length: count}, (_, i) => (
      <Cue key={i} at={Math.round(from + i * every)} sfx={sfx} gain={gain * Math.pow(decay, i)} />
    ))}
  </>
);
