import React from 'react';
import {AbsoluteFill} from 'remotion';
import {
  TransitionPresentation, TransitionPresentationComponentProps, TransitionSeries, linearTiming, springTiming,
} from '@remotion/transitions';
import {EASE} from './Camera';
import {fade} from '@remotion/transitions/fade';
import {wipe} from '@remotion/transitions/wipe';
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

/* ── push ──────────────────────────────────────────────────────────────── */

type PushProps = {axis: 'x' | 'y'; sign: 1 | -1};

/** A whip with the throw taken out of it: same travel, no blur, eased like a camera move. */
const Push: React.FC<TransitionPresentationComponentProps<PushProps>> = ({
  children, presentationDirection, presentationProgress, passedProps,
}) => {
  const {axis, sign} = passedProps;
  const entering = presentationDirection === 'entering';
  const offset = (entering ? 1 - presentationProgress : -presentationProgress) * 100 * sign;
  return (
    <AbsoluteFill
      style={{
        transform: axis === 'x' ? `translateX(${offset}%)` : `translateY(${offset}%)`,
        willChange: 'transform',
      }}
    >
      {children}
    </AbsoluteFill>
  );
};

export const push = (opts: Partial<PushProps> = {}): TransitionPresentation<PushProps> => ({
  component: Push,
  props: {axis: 'x', sign: 1, ...opts},
});

/* ── the table ───────────────────────────────────────────────────────────── */

/**
 * `cut` is a genuine zero-frame cut, not a very fast dissolve. An edit at this pace needs real cuts
 * — a film where every one of twenty joins is a crossfade has no punctuation, only commas.
 */
export type Xit = 'cut' | 'fade' | 'whip' | 'whipUp' | 'push' | 'pushUp' | 'wipe' | 'dissolve';

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
    case 'push':
      return (
        <TransitionSeries.Transition
          key={key} presentation={push({axis: 'x', sign: 1})}
          timing={linearTiming({durationInFrames: XFADE.pan, easing: EASE.glide})}
        />
      );
    case 'pushUp':
      return (
        <TransitionSeries.Transition
          key={key} presentation={push({axis: 'y', sign: 1})}
          timing={linearTiming({durationInFrames: XFADE.pan, easing: EASE.glide})}
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
  : t === 'push' || t === 'pushUp' ? XFADE.pan
  : t === 'wipe' || t === 'dissolve' ? XFADE.wide
  : XFADE.soft;
