import React from 'react';
import {interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {EASE} from './Camera';
import {ACCENT, MONO, MUTED, ON_BG, SERIF} from './theme';

/* ── entrances ───────────────────────────────────────────────────────────── */

/** The workhorse: rise and fade. Stiffer than the 0.8.9 cut's, because beats are half as long now. */
export const useRise = (delay = 0, travel = 16): React.CSSProperties => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const s = spring({frame: frame - delay, fps, config: {damping: 200, stiffness: 140}});
  return {opacity: s, transform: `translateY(${(1 - s) * travel}px)`};
};

/**
 * A masked reveal — the line is uncovered left to right rather than faded up. Reserved for titles,
 * where it gives the copy a direction of travel that agrees with the camera instead of fighting it.
 */
export const useWipe = (delay = 0, len = 14): {outer: React.CSSProperties; inner: React.CSSProperties} => {
  const frame = useCurrentFrame();
  const p = interpolate(frame - delay, [0, len], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.snap,
  });
  return {
    outer: {clipPath: `inset(-0.25em ${(1 - p) * 100}% -0.25em 0)`},
    inner: {opacity: interpolate(p, [0, 0.15], [0, 1], {extrapolateRight: 'clamp'}), display: 'block'},
  };
};

/* ── blocks ──────────────────────────────────────────────────────────────── */

export const Eyebrow: React.FC<{children: React.ReactNode; delay?: number}> = ({children, delay = 0}) => (
  <div
    style={{
      fontFamily: MONO, fontSize: 21, letterSpacing: 4.5, color: ACCENT,
      textTransform: 'uppercase', ...useRise(delay, 10),
    }}
  >
    {children}
  </div>
);

/** Multi-line titles stagger by line, so a two-line headline lands as two events, not one block. */
export const Title: React.FC<{children: string; delay?: number; size?: number; wipe?: boolean}> = ({
  children, delay = 0, size = 76, wipe = true,
}) => {
  const lines = children.split('\n');
  return (
    <div style={{fontFamily: SERIF, fontSize: size, lineHeight: 1.06, color: ON_BG}}>
      {lines.map((line, i) => (
        <TitleLine key={i} delay={delay + i * 5} wipe={wipe}>{line}</TitleLine>
      ))}
    </div>
  );
};

const TitleLine: React.FC<{children: string; delay: number; wipe: boolean}> = ({children, delay, wipe}) => {
  const w = useWipe(delay);
  const r = useRise(delay, 14);
  return wipe
    ? <span style={{display: 'block', ...w.outer}}><span style={w.inner}>{children}</span></span>
    : <span style={{display: 'block', ...r}}>{children}</span>;
};

export const Body: React.FC<{children: React.ReactNode; delay?: number; width?: number}> = ({
  children, delay = 0, width = 660,
}) => (
  <div
    style={{
      fontFamily: SERIF, fontSize: 29, lineHeight: 1.45, color: MUTED,
      maxWidth: width, fontStyle: 'italic', ...useRise(delay, 12),
    }}
  >
    {children}
  </div>
);

/** One short line, for beats too fast to hold a paragraph. Roman, not italic — it reads quicker. */
export const Line: React.FC<{children: React.ReactNode; delay?: number; width?: number}> = ({
  children, delay = 0, width = 560,
}) => (
  <div
    style={{
      fontFamily: SERIF, fontSize: 30, lineHeight: 1.35, color: MUTED,
      maxWidth: width, ...useRise(delay, 12),
    }}
  >
    {children}
  </div>
);

/**
 * The era tag under a device. The two labels occupy one slot, so they must not both be on screen at
 * once — crossfading them at 50/50 literally renders "00899". The old one leaves before the new one
 * arrives, and each moves the way the seam does.
 */
export const EraTag: React.FC<{p: number; delay?: number}> = ({p, delay = 0}) => {
  const r = useRise(delay, 10);
  const out = interpolate(p, [0, 0.42], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const inn = interpolate(p, [0.56, 1], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const base: React.CSSProperties = {
    position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center',
    fontFamily: MONO, fontSize: 20, letterSpacing: 3, textTransform: 'uppercase',
    borderRadius: 999, border: '1.5px solid',
  };
  return (
    <div style={{position: 'relative', width: 148, height: 44, ...r}}>
      <div style={{...base, opacity: out, color: MUTED, borderColor: '#38302A', transform: `translateY(${(1 - out) * -10}px)`}}>
        0.8.9
      </div>
      <div style={{...base, opacity: inn, color: ACCENT, borderColor: ACCENT, transform: `translateY(${(1 - inn) * 10}px)`}}>
        0.9
      </div>
    </div>
  );
};

export const Tag: React.FC<{label: string; accent?: boolean; delay?: number}> = ({
  label, accent = false, delay = 0,
}) => (
  <div
    style={{
      fontFamily: MONO, fontSize: 20, letterSpacing: 3, textTransform: 'uppercase',
      color: accent ? ACCENT : MUTED,
      border: `1.5px solid ${accent ? ACCENT : '#38302A'}`,
      borderRadius: 999, padding: '9px 24px', ...useRise(delay, 10),
    }}
  >
    {label}
  </div>
);

/** A number that counts. Tabular figures so the digits do not jitter as they climb. */
export const Counter: React.FC<{
  to: number; from?: number; start?: number; span?: number; size?: number; color?: string;
}> = ({to, from = 0, start = 8, span = 50, size = 180, color = ON_BG}) => {
  const frame = useCurrentFrame();
  const p = interpolate(frame, [start, start + span], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.glide,
  });
  const v = Math.round(from + (to - from) * p);
  return (
    <div style={{fontFamily: SERIF, fontSize: size, lineHeight: 1, color, fontVariantNumeric: 'tabular-nums'}}>
      {v.toLocaleString('en-US')}
    </div>
  );
};

/**
 * How many frames a beat fades over at its head and tail. `Release` provides this per beat, and
 * it is zero at every join: a beat that fades itself in and out over ten frames turns a hard cut
 * into a dip to black — the picture is darkest ON the downbeat and the new frame only arrives a
 * third of a second later — and puts a seven-frame whip between two frames at 35% brightness.
 * Every cut in the first release cut did this, and it is a large part of why they read as late.
 * Only the film's first frame fades up and its last frame fades down; the transitions do the rest.
 */
export const Edges = React.createContext<{head: number; tail: number} | null>(null);

/** Frame-accurate fade at the head and tail of a scene; see `Edges`. */
export const useEdgeFade = (durationInFrames: number, len = 10) => {
  const frame = useCurrentFrame();
  const edges = React.useContext(Edges);
  const head = edges ? edges.head : len;
  const tail = edges ? edges.tail : len;
  const a = head > 0 ? interpolate(frame, [0, head], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'}) : 1;
  const b = tail > 0
    ? interpolate(frame, [durationInFrames - tail, durationInFrames], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'})
    : 1;
  return a * b;
};
