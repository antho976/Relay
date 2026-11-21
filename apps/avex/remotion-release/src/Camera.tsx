import React from 'react';
import {Easing, interpolate, useCurrentFrame, useVideoConfig} from 'remotion';

/**
 * Springs were the 0.8.9 cut's only easing, which is why every one of its twenty beats entered
 * identically. Camera moves want curves you can aim: `glide` decelerates into a rest, `drift` is
 * near-linear so a long slow move never stalls mid-shot, `snap` arrives fast and settles late.
 */
export const EASE = {
  glide:  Easing.bezier(0.32, 0.00, 0.20, 1.00),
  drift:  Easing.bezier(0.42, 0.00, 0.58, 1.00),
  snap:   Easing.bezier(0.16, 1.00, 0.30, 1.00),
  rush:   Easing.bezier(0.70, 0.00, 0.84, 0.00),
  /** For the accent edge: decisive, but on screen long enough to read. `snap` is not — it is at
   *  0.97 by the halfway frame, which turned an 26-frame sweep into a two-frame flicker. */
  sweep:  Easing.bezier(0.55, 0.02, 0.22, 1.00),
  linear: (t: number) => t,
} as const;

export type Span = [number, number];

export type Shot = {
  /** scale, 1 = framed as laid out */
  z?: Span;
  /** translation in composition pixels */
  x?: Span;
  y?: Span;
  /** degrees — keep tiny, this is a product film not a music video */
  rot?: Span;
  start?: number;
  span?: number;
  ease?: (t: number) => number;
};

export const useShot = (shot?: Shot) => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  if (!shot) return {t: 0, style: {} as React.CSSProperties};
  const start = shot.start ?? 0;
  const span = shot.span ?? durationInFrames - start;
  const ease = shot.ease ?? EASE.glide;
  const t = interpolate(frame, [start, start + Math.max(1, span)], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: ease,
  });
  const at = (s?: Span, d = 0) => (s ? s[0] + (s[1] - s[0]) * t : d);
  const z = at(shot.z, 1);
  const x = at(shot.x, 0);
  const y = at(shot.y, 0);
  const rot = at(shot.rot, 0);
  return {
    t,
    style: {
      transform: `translate(${x}px, ${y}px) scale(${z}) rotate(${rot}deg)`,
      transformOrigin: 'center center',
    } as React.CSSProperties,
  };
};

/** Moves the whole frame. Everything inside travels together, so the shot reads as one camera. */
export const Camera: React.FC<{shot?: Shot; children: React.ReactNode}> = ({shot, children}) => {
  const {style} = useShot(shot);
  return <div style={{position: 'absolute', inset: 0, ...style, willChange: 'transform'}}>{children}</div>;
};

/**
 * Depth. The copy drifting a little against the device is the cheapest way to stop a flat 2D layout
 * reading as a slide; `depth` is a multiplier on the camera's own travel, negative for counter-drift.
 */
export const useParallax = (depth: number, shot?: Shot): React.CSSProperties => {
  const {t} = useShot(shot);
  return {transform: `translateX(${t * depth}px)`};
};
