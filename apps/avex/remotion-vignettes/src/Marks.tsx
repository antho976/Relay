import React from 'react';
import {ACCENT, ACCENT_GLOW, MUTED} from './theme';

/**
 * The vocabulary all three vignettes are written in.
 *
 * **One set = one block.** Counting the sets out is what makes these read as real work: a row of
 * three blocks next to a row of five has a ragged right edge that says "three sets, then five",
 * where the plain varying-width bar this replaced just read as a loading skeleton. It is also the
 * only honest way to spend accent here — §5 wants it on data, at size, in few places, and a set IS
 * the datum the whole app is built on.
 *
 * Every row is the same shape in all three cards: mono UPPERCASE text, then its blocks. The cards
 * differ only in WHAT the text says, how the rows are ARRANGED, and the RHYTHM they land in — which
 * is the entire point of the step: an aligned week of named days that arrives all at once · one day's
 * exercises added a row at a time by a `+` · loose day stamps landing wherever, in no order.
 */

export const SET_W = 38;
export const SET_H = 14; // ~3.5dp — §10's thin rounded bar, not a chunky lozenge
export const SET_GAP = 10;

/** Width of a row of `sets` blocks — used to centre/scatter rows without measuring the DOM. */
export const setsWidth = (sets: number, scale = 1): number =>
  (sets * SET_W + (sets - 1) * SET_GAP) * scale;

/** Mono micro-label (DESIGN §6): UPPERCASE, letter-spaced, muted. ~10sp at this canvas's 4x. */
export const LABEL_SIZE = 40;
export const LABEL_TRACK = 7;

export const MonoLabel: React.FC<{
  children: string;
  opacity: number;
  scale?: number;
  color?: string;
}> = ({children, opacity, scale = 1, color = MUTED}) => (
  <div
    style={{
      fontFamily: 'monospace',
      fontSize: LABEL_SIZE * scale,
      lineHeight: `${LABEL_SIZE * scale}px`,
      letterSpacing: LABEL_TRACK * scale,
      color,
      opacity,
      whiteSpace: 'nowrap',
    }}
  >
    {children}
  </div>
);

/**
 * A row of `sets` blocks. [alphaAt] is asked per block so a caller can count them out one at a time
 * (Generated.tsx tallies the whole week that way) or land the row whole. [glow] blooms under a block
 * that has just arrived and fades as it settles — the only decoration in the set, and it is spent on
 * the moment the work appears.
 */
export const SetRow: React.FC<{
  sets: number;
  alphaAt: (index: number) => number;
  glow?: number;
  scale?: number;
  color?: string;
}> = ({sets, alphaAt, glow = 0, scale = 1, color = ACCENT}) => (
  <div style={{display: 'flex', gap: SET_GAP * scale}}>
    {Array.from({length: sets}, (_, i) => {
      const alpha = alphaAt(i);
      if (alpha <= 0) return null;
      return (
        <div
          key={i}
          style={{
            width: SET_W * scale,
            height: SET_H * scale,
            borderRadius: (SET_H * scale) / 2,
            background: color,
            opacity: alpha,
            boxShadow: glow > 0 ? `0 0 ${26 * glow * scale}px ${ACCENT_GLOW}` : undefined,
          }}
        />
      );
    })}
  </div>
);
