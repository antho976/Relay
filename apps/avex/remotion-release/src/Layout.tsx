import React from 'react';
import {AbsoluteFill, interpolate, useCurrentFrame} from 'remotion';
import {ACCENT, MONO, MUTED, ON_BG} from './theme';

/**
 * One noise tile, rasterised once by the browser and then merely re-positioned each frame. Animating
 * feTurbulence's seed instead would re-run the filter 2,700 times over a 90-second render.
 */
const NOISE =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='220' height='220'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.75' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='220' height='220' filter='url(%23n)' opacity='0.55'/%3E%3C/svg%3E\")";

export const Grain: React.FC<{opacity?: number}> = ({opacity = 0.045}) => {
  const frame = useCurrentFrame();
  // A prime-ish step keeps the tile from visibly cycling back to where it started.
  const x = (frame * 37) % 220;
  const y = (frame * 53) % 220;
  return (
    <div
      style={{
        position: 'absolute', inset: -220, pointerEvents: 'none', opacity,
        backgroundImage: NOISE, backgroundPosition: `${x}px ${y}px`, mixBlendMode: 'overlay',
      }}
    />
  );
};

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
      paddingTop: 22, paddingBottom: 46, boxSizing: 'border-box',
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

/**
 * The subject fills the frame and the copy sits on a scrim over it.
 *
 * Both scrims are load-bearing. The bottom one has to beat live footage — a 150 in 200pt white will
 * read straight through a polite gradient — so it is tall and nearly opaque where the copy sits. The
 * top one exists because a full-bleed crop lands wherever it lands, and a headline sliced in half by
 * the frame edge reads as a mistake rather than as a crop; under a scrim it reads as depth.
 */
export const Full: React.FC<{copy: React.ReactNode; children: React.ReactNode}> = ({copy, children}) => (
  <div style={{position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center'}}>
    {children}
    <div
      style={{
        position: 'absolute', left: 0, right: 0, top: 0, height: 190,
        background: 'linear-gradient(to bottom, rgba(10,8,6,0.82) 0%, rgba(10,8,6,0.34) 46%, rgba(10,8,6,0) 100%)',
      }}
    />
    <div
      style={{
        position: 'absolute', left: 0, right: 0, bottom: 0, height: 500,
        background:
          'linear-gradient(to top, rgba(10,8,6,0.97) 0%, rgba(10,8,6,0.94) 26%, rgba(10,8,6,0.66) 58%, rgba(10,8,6,0) 100%)',
      }}
    />
    <div style={{position: 'absolute', left: 104, bottom: 116, display: 'flex', flexDirection: 'column', gap: 18}}>
      {copy}
    </div>
  </div>
);

/* ── the rail ────────────────────────────────────────────────────────────── */

/**
 * A chapter rail that persists across every cut, drawn once over the whole film rather than per
 * beat. It does the job an edit this fast otherwise cannot: tells you where you are, and gives the
 * twenty cuts one continuous object so they read as one film instead of twenty slides.
 */
export type Act = {at: number; label: string};

export const Rail: React.FC<{acts: Act[]; total: number; lead?: number; tail?: number}> = ({
  acts, total, lead = 0, tail = 0,
}) => {
  const frame = useCurrentFrame();
  const visible = frame >= lead && frame <= total - tail;
  const p = interpolate(frame, [0, total], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const o = interpolate(frame, [lead, lead + 12, total - tail - 12, total - tail], [0, 1, 1, 0], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });
  const act = acts.filter((a) => a.at <= frame).pop();

  if (!visible) return null;
  return (
    <AbsoluteFill style={{pointerEvents: 'none', opacity: o}}>
      <div style={{position: 'absolute', top: 54, left: 104, display: 'flex', gap: 14, alignItems: 'center'}}>
        <div style={{width: 9, height: 9, borderRadius: 999, background: ACCENT}} />
        <div style={{fontFamily: MONO, fontSize: 17, letterSpacing: 4, color: ON_BG, opacity: 0.75}}>AVEX 0.9</div>
      </div>
      <div style={{position: 'absolute', bottom: 52, left: 104, right: 104, display: 'flex', alignItems: 'center', gap: 26}}>
        <div style={{fontFamily: MONO, fontSize: 16, letterSpacing: 4, color: MUTED, opacity: 0.7, minWidth: 210}}>
          {act ? act.label.toUpperCase() : ''}
        </div>
        <div style={{flex: 1, height: 1, background: '#38302A', position: 'relative'}}>
          <div style={{position: 'absolute', left: 0, top: 0, height: 1, width: `${p * 100}%`, background: ACCENT, opacity: 0.85}} />
          {acts.map((a, i) => (
            <div
              key={i}
              style={{
                position: 'absolute', left: `${(a.at / total) * 100}%`, top: -2,
                width: 1, height: 5, background: '#38302A',
              }}
            />
          ))}
        </div>
      </div>
    </AbsoluteFill>
  );
};
