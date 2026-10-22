// Before and after in one place: the old version underneath, the new one revealed by a coloured
// edge sweeping across, and a version pill that hands one name to the other. One screen that
// changes reads as an update; two half-size screens side by side read as spot-the-difference.
import React from 'react';
import { interpolate, useCurrentFrame } from 'remotion';
import { easeInOut } from './motion';

const clamp = (v: number) => Math.max(0, Math.min(1, v));

/** 0 → 1 over `duration` frames from `at`, eased in and out: how far a sweep has come. */
export const useSweep = (at: number, duration = 20): number => {
  const frame = useCurrentFrame();
  return interpolate(frame, [at, at + Math.max(1, duration)], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeInOut });
};

/** `before` underneath, `after` revealed by an edge sweeping across it, as `progress` goes 0 → 1
 * (or from frame `at` over `duration`). `direction` is where the edge travels: `right` (default)
 * or `down`. Fill a phone's screen with it, or any box: it takes its parent's size. */
export const BeforeAfter: React.FC<{
  before: React.ReactNode;
  after: React.ReactNode;
  /** The frame the sweep starts (in the sequence it is in), when `progress` is not given. */
  at?: number;
  duration?: number;
  progress?: number;
  direction?: 'right' | 'down';
  /** The edge's colour; default a clear red. */
  color?: string;
  /** The edge's width in px. */
  edge?: number;
  /** A soft wash of the edge's colour trailing it. */
  wash?: boolean;
  id?: string;
}> = ({ before, after, at = 0, duration = 20, progress, direction = 'right', color = '#FF3B30', edge = 6, wash = true, id }) => {
  const swept = useSweep(at, duration);
  const p = clamp(progress ?? swept);
  const q = p * 100;
  const down = direction === 'down';
  const moving = p > 0.001 && p < 0.999;
  return (
    <div data-id={id} style={{ position: 'absolute', inset: 0, overflow: 'hidden' }}>
      <div style={{ position: 'absolute', inset: 0 }}>{before}</div>
      <div style={{ position: 'absolute', inset: 0, clipPath: down ? `inset(0 0 ${100 - q}% 0)` : `inset(0 ${100 - q}% 0 0)` }}>{after}</div>
      {moving && wash ? (
        <div style={{
          position: 'absolute', ...(down ? { left: 0, right: 0, top: 0, height: `${q}%` } : { top: 0, bottom: 0, left: 0, width: `${q}%` }),
          background: `linear-gradient(${down ? 'to top' : 'to left'}, ${color}38 0%, ${color}00 22%)`,
        }} />
      ) : null}
      {moving ? (
        <div aria-hidden="true" style={{
          position: 'absolute', ...(down ? { left: 0, right: 0, top: `${q}%`, height: edge, marginTop: -edge / 2 } : { top: 0, bottom: 0, left: `${q}%`, width: edge, marginLeft: -edge / 2 }),
          background: color, boxShadow: `0 0 ${edge * 3}px ${color}`,
        }} />
      ) : null}
    </div>
  );
};

/** A version tag that hands over: `from` leaves before `to` arrives, as `progress` goes 0 → 1
 * (pass the same progress as the `BeforeAfter` it labels). The new one is filled. */
export const VersionPill: React.FC<{
  from: string;
  to: string;
  progress: number;
  color?: string;
  textColor?: string;
  muted?: string;
  font?: string;
  size?: number;
  id?: string;
}> = ({ from, to, progress, color = '#D83838', textColor = '#FFFFFF', muted = '#BFB6AA', font = '"JetBrains Mono", monospace', size = 30, id = 'version' }) => {
  const out = clamp((0.44 - progress) / 0.44);
  const inn = clamp((progress - 0.56) / 0.44);
  const pill = (text: string, o: number, filled: boolean) => (
    <div style={{
      position: 'absolute', left: '50%', top: 0, transform: `translateX(-50%) translateY(${(1 - o) * size * 0.35}px)`, opacity: o,
      fontFamily: font, fontSize: size, letterSpacing: size * 0.12, padding: `${size * 0.25}px ${size * 0.65}px`, borderRadius: size * 0.15,
      whiteSpace: 'nowrap', color: filled ? textColor : muted, background: filled ? color : 'transparent',
      border: filled ? 'none' : `2px solid ${muted}80`,
    }}>{text}</div>
  );
  return (
    <div data-id={id} style={{ position: 'relative', height: size * 1.9, width: Math.max(from.length, to.length) * size * 0.75 + size * 1.6 }}>
      {pill(from, out, false)}
      {pill(to, inn, true)}
    </div>
  );
};
