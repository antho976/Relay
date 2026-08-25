import React from 'react';
import {Img, OffthreadVideo, interpolate, staticFile, useCurrentFrame, useVideoConfig} from 'remotion';
import {ACCENT, OUTLINE, SHOT_AR, SURFACE} from './theme';
import {EASE} from './Camera';

/* ── media ───────────────────────────────────────────────────────────────── */

/** A source: a capture from `public/cfr/`, or a still. Both fill the device box. */
export type Clip = {
  src?: string;
  still?: string;
  /** first source frame to show */
  start?: number;
  /** <1 slows the clip, >1 speeds it — used to make two unequal scrolls finish together */
  rate?: number;
};

export const Media: React.FC<{clip: Clip}> = ({clip}) => {
  const fill: React.CSSProperties = {width: '100%', height: '100%', objectFit: 'cover', display: 'block'};
  if (clip.src) {
    return (
      <OffthreadVideo
        src={staticFile(clip.src)}
        trimBefore={clip.start ?? 0}
        playbackRate={clip.rate ?? 1}
        muted
        style={fill}
      />
    );
  }
  if (clip.still) return <Img src={staticFile(clip.still)} style={fill} />;
  return null;
};

/* ── focus: the camera inside the screen ─────────────────────────────────── */

/**
 * The captures are 1080×2400 but a phone drawn at 800px tall shows that detail at a twelfth of its
 * device size — a chip, a sparkline, a truncated number is simply unreadable. `focus` flies a window
 * over the source instead of showing all of it: `z` is the zoom, `x`/`y` the point held at centre,
 * both normalised 0–1. This is where most of the film's movement comes from.
 */
export type Focus = {x?: number; y?: number; z?: number};
export type FocusMove = {from: Focus; to?: Focus; start?: number; span?: number; ease?: (t: number) => number};

const WHOLE: Required<Focus> = {x: 0.5, y: 0.5, z: 1};

/** Keep the window inside the source, so a zoomed pan can never reveal empty box. */
const clampFocus = (f: Required<Focus>): Required<Focus> => {
  const z = Math.max(1, f.z);
  const half = 0.5 / z;
  return {
    z,
    x: Math.min(1 - half, Math.max(half, f.x)),
    y: Math.min(1 - half, Math.max(half, f.y)),
  };
};

export const focusStyle = (f: Required<Focus>): React.CSSProperties => {
  const c = clampFocus(f);
  // transform-origin is the layer's own centre, so scale∘translate puts the point (x,y) dead centre.
  return {transform: `scale(${c.z}) translate(${(0.5 - c.x) * 100}%, ${(0.5 - c.y) * 100}%)`};
};

export const useFocus = (move?: FocusMove): React.CSSProperties => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  if (!move) return {};
  const from = {...WHOLE, ...move.from};
  const to = {...from, ...(move.to ?? {})};
  const start = move.start ?? 0;
  const span = move.span ?? durationInFrames - start;
  const ease = move.ease ?? EASE.glide;
  const t = interpolate(frame, [start, start + Math.max(1, span)], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: ease,
  });
  return focusStyle({
    x: from.x + (to.x - from.x) * t,
    y: from.y + (to.y - from.y) * t,
    z: from.z + (to.z - from.z) * t,
  });
};

/* ── the seam ────────────────────────────────────────────────────────────── */

/**
 * One device, two eras. The brief was explicitly "no two renders" — a side-by-side turns a release
 * into a spot-the-difference diagram, and it halves the size of both phones. So the two captures are
 * stacked in a single body and an accent edge sweeps across: behind the line is 0.8.9, in front of it
 * is 0.9. The line is AccentRed on purpose — navy becoming red is literally what the release did.
 */
export type SeamDir = 'ltr' | 'rtl' | 'ttb' | 'btt';

export const seamClip = (dir: SeamDir, p: number): string => {
  const q = Math.max(0, Math.min(1, p)) * 100;
  if (dir === 'ltr') return `inset(0 ${100 - q}% 0 0)`;
  if (dir === 'rtl') return `inset(0 0 0 ${100 - q}%)`;
  if (dir === 'ttb') return `inset(0 0 ${100 - q}% 0)`;
  return `inset(${100 - q}% 0 0 0)`;
};

const SeamEdge: React.FC<{dir: SeamDir; p: number; thickness: number}> = ({dir, p, thickness}) => {
  if (p <= 0.001 || p >= 0.999) return null;
  const vertical = dir === 'ltr' || dir === 'rtl';
  const pos = `${(dir === 'rtl' || dir === 'btt' ? 1 - p : p) * 100}%`;
  const glowDir = dir === 'ltr' ? 'to left' : dir === 'rtl' ? 'to right' : dir === 'ttb' ? 'to top' : 'to bottom';
  return (
    <>
      {/* the light the edge throws back over the era it has just replaced */}
      <div
        style={{
          position: 'absolute',
          ...(vertical
            ? {top: 0, bottom: 0, left: 0, width: pos}
            : {left: 0, right: 0, top: 0, height: pos}),
          background: `linear-gradient(${glowDir}, ${ACCENT}2E 0%, ${ACCENT}00 22%)`,
          pointerEvents: 'none',
        }}
      />
      <div
        style={{
          position: 'absolute',
          ...(vertical
            ? {top: 0, bottom: 0, left: pos, width: thickness, marginLeft: -thickness / 2}
            : {left: 0, right: 0, top: pos, height: thickness, marginTop: -thickness / 2}),
          background: ACCENT,
          boxShadow: `0 0 ${thickness * 7}px ${thickness * 1.6}px ${ACCENT}66`,
          pointerEvents: 'none',
        }}
      />
    </>
  );
};

export const Seam: React.FC<{
  a: React.ReactNode;      // 0.8.9 — underneath
  b: React.ReactNode;      // 0.9 — revealed by the edge
  p: number;               // 0..1
  dir?: SeamDir;
  thickness?: number;
}> = ({a, b, p, dir = 'ltr', thickness = 4}) => (
  <>
    <div style={{position: 'absolute', inset: 0}}>{a}</div>
    <div style={{position: 'absolute', inset: 0, clipPath: seamClip(dir, p)}}>{b}</div>
    <SeamEdge dir={dir} p={p} thickness={thickness} />
  </>
);

/* ── the body ────────────────────────────────────────────────────────────── */

/**
 * The phone itself. `children` is deliberate: a beat can put a real capture, a still, or a natively
 * rebuilt Compose screen inside the same body, and they all read as the same device.
 */
export const Device: React.FC<{
  height: number;
  clip?: Clip;
  focus?: FocusMove;
  bare?: boolean;
  glow?: boolean;
  children?: React.ReactNode;
  /** drawn over the screen, in the screen's own coordinates — callouts, never chrome */
  overlay?: React.ReactNode;
  style?: React.CSSProperties;
}> = ({height, clip, focus, bare = false, glow = false, children, overlay, style}) => {
  const width = height / SHOT_AR;
  const radius = bare ? 20 : 46;
  const bezel = bare ? 0 : Math.round(height * 0.011);
  const f = useFocus(focus);

  return (
    <div
      style={{
        width: width + bezel * 2,
        height: height + bezel * 2,
        borderRadius: radius + bezel,
        background: bare ? 'transparent' : '#000',
        padding: bezel,
        boxSizing: 'border-box',
        boxShadow: bare
          ? '0 24px 60px rgba(0,0,0,0.55)'
          : `0 0 0 1.5px ${OUTLINE}, 0 30px 80px rgba(0,0,0,0.65)${glow ? `, 0 0 90px -10px ${ACCENT}55` : ''}`,
        flex: '0 0 auto',
        ...style,
      }}
    >
      <div
        style={{
          width, height, borderRadius: radius, overflow: 'hidden',
          background: SURFACE, position: 'relative',
        }}
      >
        <div style={{position: 'absolute', inset: 0, ...f}}>
          {children ?? (clip ? <Media clip={clip} /> : null)}
        </div>
        {overlay ? <div style={{position: 'absolute', inset: 0, pointerEvents: 'none'}}>{overlay}</div> : null}
      </div>
    </div>
  );
};
