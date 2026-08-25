import React from 'react';
import {AbsoluteFill} from 'remotion';
import {
  TransitionPresentation, TransitionPresentationComponentProps, TransitionSeries, linearTiming, springTiming,
} from '@remotion/transitions';
import {fade} from '@remotion/transitions/fade';
import {wipe} from '@remotion/transitions/wipe';
import {ACCENT} from './theme';
import {XFADE} from './theme';

/* ── whip ────────────────────────────────────────────────────────────────── */

type WhipProps = {axis: 'x' | 'y'; sign: 1 | -1; blur: number};

/**
 * A fast lateral throw with the blur the throw would actually cause. Remotion's stock `slide` moves
 * the frames but keeps them razor sharp, which at seven frames reads as a slide projector; the blur
 * peaks at mid-transition, where the velocity does, and is gone by the time the new beat settles.
 */
const Whip: React.FC<TransitionPresentationComponentProps<WhipProps>> = ({
  children, presentationDirection, presentationProgress, passedProps,
}) => {
  const {axis, sign, blur} = passedProps;
  const entering = presentationDirection === 'entering';
  const p = presentationProgress;
  const offset = (entering ? (1 - p) : -p) * 100 * sign;
  const v = Math.sin(Math.PI * p);
  return (
    <AbsoluteFill
      style={{
        transform: axis === 'x' ? `translateX(${offset}%)` : `translateY(${offset}%)`,
        filter: `blur(${(v * blur).toFixed(2)}px)`,
        willChange: 'transform, filter',
      }}
    >
      {children}
    </AbsoluteFill>
  );
};

export const whip = (opts: Partial<WhipProps> = {}): TransitionPresentation<WhipProps> => ({
  component: Whip,
  props: {axis: 'x', sign: 1, blur: 9, ...opts},
});

/* ── the accent sweep ────────────────────────────────────────────────────── */

type SweepProps = {sign: 1 | -1};

/**
 * The seam, promoted to a cut. Where a beat inside the film uses an accent edge to turn 0.8.9 into
 * 0.9 in one device, this uses the same edge to turn one beat into the next — so the film's one
 * recurring gesture also does its section breaks, instead of borrowing a stock wipe for them.
 */
const Sweep: React.FC<TransitionPresentationComponentProps<SweepProps>> = ({
  children, presentationDirection, presentationProgress, passedProps,
}) => {
  const {sign} = passedProps;
  const p = presentationProgress;
  if (presentationDirection === 'exiting') return <AbsoluteFill>{children}</AbsoluteFill>;
  const q = p * 100;
  const clip = sign === 1 ? `inset(0 ${100 - q}% 0 0)` : `inset(0 0 0 ${100 - q}%)`;
  const edge = `${sign === 1 ? q : 100 - q}%`;
  return (
    <>
      <AbsoluteFill style={{clipPath: clip}}>{children}</AbsoluteFill>
      {p > 0.01 && p < 0.99 ? (
        <AbsoluteFill style={{pointerEvents: 'none'}}>
          <div
            style={{
              position: 'absolute', top: 0, bottom: 0, left: edge, width: 4, marginLeft: -2,
              background: ACCENT, boxShadow: `0 0 40px 8px ${ACCENT}55`,
            }}
          />
        </AbsoluteFill>
      ) : null}
    </>
  );
};

export const sweep = (opts: Partial<SweepProps> = {}): TransitionPresentation<SweepProps> => ({
  component: Sweep,
  props: {sign: 1, ...opts},
});

/* ── the table ───────────────────────────────────────────────────────────── */

/**
 * `cut` is a genuine zero-frame cut, not a very fast dissolve. An edit at this pace needs real cuts
 * — a film where every one of twenty joins is a crossfade has no punctuation, only commas.
 */
export type Xit = 'cut' | 'fade' | 'whip' | 'whipUp' | 'sweep' | 'sweepBack' | 'wipe' | 'dissolve';

export const transition = (t: Xit, key: string): React.ReactNode | null => {
  switch (t) {
    case 'cut':
      return null;
    case 'whip':
      return (
        <TransitionSeries.Transition
          key={key} presentation={whip({axis: 'x', sign: 1, blur: 10})}
          timing={linearTiming({durationInFrames: XFADE.quick})}
        />
      );
    case 'whipUp':
      return (
        <TransitionSeries.Transition
          key={key} presentation={whip({axis: 'y', sign: 1, blur: 8})}
          timing={linearTiming({durationInFrames: XFADE.quick})}
        />
      );
    case 'sweep':
      return (
        <TransitionSeries.Transition
          key={key} presentation={sweep({sign: 1})}
          timing={linearTiming({durationInFrames: XFADE.soft})}
        />
      );
    case 'sweepBack':
      return (
        <TransitionSeries.Transition
          key={key} presentation={sweep({sign: -1})}
          timing={linearTiming({durationInFrames: XFADE.soft})}
        />
      );
    case 'wipe':
      return (
        <TransitionSeries.Transition
          key={key} presentation={wipe({direction: 'from-bottom-right'})}
          timing={springTiming({config: {damping: 200}, durationInFrames: XFADE.wide})}
        />
      );
    case 'dissolve':
      return (
        <TransitionSeries.Transition
          key={key} presentation={fade()} timing={linearTiming({durationInFrames: XFADE.wide})}
        />
      );
    default:
      return (
        <TransitionSeries.Transition
          key={key} presentation={fade()} timing={linearTiming({durationInFrames: XFADE.soft})}
        />
      );
  }
};

/** How many frames a join eats, so the running time can be computed rather than guessed. */
export const overlap = (t: Xit): number =>
  t === 'cut' ? 0
  : t === 'whip' || t === 'whipUp' ? XFADE.quick
  : t === 'wipe' || t === 'dissolve' ? XFADE.wide
  : XFADE.soft;
