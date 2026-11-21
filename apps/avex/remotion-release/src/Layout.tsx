import React from 'react';
import {AbsoluteFill, interpolate, useCurrentFrame} from 'remotion';
import {ACCENT, MONO, ON_BG} from './theme';

/**
 * One noise tile, rasterised once by the browser and then merely re-positioned each frame. Animating
 * feTurbulence's seed instead would re-run the filter 2,700 times over a 90-second render.
 */
const NOISE =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='220' height='220'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.75' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='220' height='220' filter='url(%23n)' opacity='0.55'/%3E%3C/svg%3E\")";

/**
 * Static, deliberately.
 *
 * This used to translate the tile by 37px horizontally and 53px vertically per frame, which with a
 * 220px tile wraps every 6 and 4 frames — a 5-7 Hz strobe across the whole frame. That is almost
 * certainly what read as everything gently bouncing. Film grain that moves has to be regenerated per
 * frame, not slid around; a still tile at low opacity does the job of breaking up flat black without
 * putting a pulse under the picture.
 */
export const Grain: React.FC<{opacity?: number}> = ({opacity = 0.035}) => (
  <div
    style={{
      position: 'absolute', inset: 0, pointerEvents: 'none', opacity,
      backgroundImage: NOISE, mixBlendMode: 'overlay',
    }}
  />
);

/** The ground every beat sits on: a lifted centre so the dark plate never reads as flat black. */
export const Plate: React.FC<{children: React.ReactNode; grain?: boolean}> = ({children, grain = true}) => (
  <div
    style={{
      position: 'absolute', inset: 0,
      background: 'radial-gradient(120% 90% at 50% 18%, #17120E 0%, #110F0C 55%, #0A0806 100%)',
      display: 'flex', alignItems: 'center', justifyContent: 'center', overflow: 'hidden',
    }}
  >
    {children}
    {grain ? <Grain /> : null}
  </div>
);

/* ── stages ──────────────────────────────────────────────────────────────── */

/**
 * Copy on one side, the subject on the other. `flip` mirrors it — the 0.8.9 cut put the copy left
 * on all twenty beats, and by the fourth beat the eye stops travelling and the film goes flat.
 */
export const Split: React.FC<{
  copy: React.ReactNode;
  children: React.ReactNode;
  flip?: boolean;
  copyWidth?: number;
  gap?: number;
}> = ({copy, children, flip = false, copyWidth = 700, gap = 56}) => (
  <div
    style={{
      display: 'flex', width: '100%', height: '100%', alignItems: 'center',
      flexDirection: flip ? 'row-reverse' : 'row',
      paddingLeft: flip ? 0 : 104, paddingRight: flip ? 104 : 0, gap,
      paddingTop: 26, paddingBottom: 26, boxSizing: 'border-box',
    }}
  >
    <div style={{flex: `0 0 ${copyWidth}px`, display: 'flex', flexDirection: 'column', gap: 22}}>
      {copy}
    </div>
    <div style={{flex: 1, display: 'flex', justifyContent: 'center', alignItems: 'center', minWidth: 0}}>
      {children}
    </div>
  </div>
);

/** Everything stacked and centred — title cards, the counter, the tab swap. */
export const Center: React.FC<{children: React.ReactNode; gap?: number; width?: number}> = ({
  children, gap = 26, width,
}) => (
  <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap, width, textAlign: 'center'}}>
    {children}
  </div>
);

/* ── the mark ────────────────────────────────────────────────────────────── */

/**
 * The only chrome that persists across every cut. It started as a full rail with an act label and a
 * progress line along the bottom; the line drew the eye away from the phone on every beat and the
 * act names read as a table of contents nobody asked for, so both are gone. What is left is a
 * standing mark, which is all a ninety-second film needs to say whose film it is.
 */
export const Mark: React.FC<{total: number; lead?: number; tail?: number; label?: string}> = ({
  total, lead = 0, tail = 0, label = 'AVEX 0.9',
}) => {
  const frame = useCurrentFrame();
  const o = interpolate(frame, [lead, lead + 12, total - tail - 12, total - tail], [0, 1, 1, 0], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });
  if (o <= 0) return null;
  return (
    <AbsoluteFill style={{pointerEvents: 'none', opacity: o}}>
      <div style={{position: 'absolute', top: 54, left: 104, display: 'flex', gap: 14, alignItems: 'center'}}>
        <div style={{width: 9, height: 9, borderRadius: 999, background: ACCENT}} />
        <div style={{fontFamily: MONO, fontSize: 17, letterSpacing: 4, color: ON_BG, opacity: 0.62}}>{label}</div>
      </div>
    </AbsoluteFill>
  );
};
