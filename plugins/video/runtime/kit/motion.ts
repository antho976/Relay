// Timing helpers: how far into an entrance an element is, and the styles that entrance draws.
// Every helper is a pure function of the frame, as Remotion needs: no timers, no CSS animations.
import type React from 'react';
import { Easing, interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';

/** Relay's ease out: fast, then settling. */
export const easeOut = Easing.bezier(0.16, 1, 0.3, 1);
/** For things that leave: slow, then gone. */
export const easeIn = Easing.bezier(0.7, 0, 0.84, 0);
export const easeInOut = Easing.bezier(0.65, 0, 0.35, 1);

export type Timing = {
  /** Frames to wait before it starts. */
  delay?: number;
  /** Frames it takes. */
  duration?: number;
};

/** 0 → 1 over `duration` frames (default 18) after `delay`, eased out; held at both ends. */
export const useProgress = ({ delay = 0, duration = 18 }: Timing = {}): number => {
  const frame = useCurrentFrame();
  return interpolate(frame - delay, [0, Math.max(1, duration)], [0, 1], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
    easing: easeOut,
  });
};

/** A spring 0 → 1 that starts after `delay` frames; `damping` 200 settles without bouncing,
 * 12 bounces. */
export const useSpring = (delay = 0, damping = 200, durationInFrames?: number): number => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return spring({ frame: frame - delay, fps, config: { damping }, durationInFrames });
};

/** 1 → 0 over the last `duration` frames before `end` (a frame of the sequence it is in). */
export const useExit = (end: number, duration = 12): number => {
  const frame = useCurrentFrame();
  return interpolate(frame, [end - duration, end], [1, 0], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
    easing: easeIn,
  });
};

/** Frames in `seconds` at the composition's rate. */
export const useSeconds = (): ((seconds: number) => number) => {
  const { fps } = useVideoConfig();
  return (seconds: number) => Math.round(seconds * fps);
};

/** The delay of item `index` in a list that enters one after another, `step` frames apart. */
export const stagger = (index: number, step = 4, base = 0): number => base + index * step;

/** Fades in and rises `distance` px as `p` goes 0 → 1. */
export const rise = (p: number, distance = 40): React.CSSProperties => ({
  opacity: p,
  transform: `translateY(${(1 - p) * distance}px)`,
});

/** Fades in and grows from `from` of its size. */
export const pop = (p: number, from = 0.85): React.CSSProperties => ({
  opacity: Math.min(1, p * 1.6),
  transform: `scale(${from + (1 - from) * p})`,
});

/** Slides in `distance` px from a side. */
export const slide = (p: number, side: 'left' | 'right' | 'up' | 'down' = 'left', distance = 80): React.CSSProperties => {
  const d = (1 - p) * distance;
  const [x, y] = side === 'left' ? [-d, 0] : side === 'right' ? [d, 0] : side === 'up' ? [0, -d] : [0, d];
  return { opacity: p, transform: `translate(${x}px, ${y}px)` };
};

/** Sharpens out of a blur as `p` goes 0 → 1. */
export const focus = (p: number, blur = 16): React.CSSProperties => ({
  opacity: p,
  filter: `blur(${(1 - p) * blur}px)`,
});

/** A value that goes from `from` to `to` as `p` goes 0 → 1. */
export const mix = (p: number, from: number, to: number): number => from + (to - from) * p;
