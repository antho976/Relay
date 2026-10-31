// Footage from a stream or a game. A camera over a source video: a crop that can move and zoom.
// A stream's vertical layout: the facecam over the gameplay. Cuts that drop dead air, with the
// captions moved to match. Punch-ins on a reaction. The marks a guide draws over a game: a ring
// with a label, an arrow, a numbered step, an opening hook.
// Times named `ms` are in the source video's milliseconds. Times named `at` or `frames` are
// frames of the composition.
import type { Caption } from '@remotion/captions';
import React, { createContext, useContext } from 'react';
import { AbsoluteFill, interpolate, OffthreadVideo, Sequence, useCurrentFrame, useVideoConfig } from 'remotion';
import { easeIn, easeInOut, easeOut, pop, useExit, useProgress, useSpring } from './motion';
import { useSafeArea } from './safe';
import { theme } from './theme';

/** A rectangle in the source video's pixels. */
export type Rect = { x: number; y: number; w: number; h: number };
export type Size = { width: number; height: number };
/** Where the camera looks from frame `at` on. `move` is how it gets there from the key before:
 * `ease` glides there (the default), `cut` jumps. `frames` is how long the glide takes (default 18;
 * 0 is a cut). */
export type ViewKey = { at: number; view: Rect; move?: 'ease' | 'cut'; frames?: number };
/** Spans of the source to keep, in order: `[startMs, endMs]`. Everything between them is cut. */
export type Keep = [number, number][];

const whole = (s: Size): Rect => ({ x: 0, y: 0, w: s.width, h: s.height });

/** `view` grown to `box`'s shape about its centre (so the box is covered, nothing stretched),
 * zoomed in by `zoom`, and moved back inside the source where it can be. */
export const fitView = (view: Rect, box: Size, source: Size, zoom = 1): Rect => {
  const aspect = box.width / box.height;
  let w = view.w;
  let h = view.h;
  if (w / h > aspect) h = w / aspect; else w = h * aspect;
  w /= zoom;
  h /= zoom;
  const cx = view.x + view.w / 2;
  const cy = view.y + view.h / 2;
  const clamp = (v: number, size: number, total: number) => (size >= total ? (total - size) / 2 : Math.min(Math.max(v, 0), total - size));
  return { x: clamp(cx - w / 2, w, source.width), y: clamp(cy - h / 2, h, source.height), w, h };
};

/** Where the camera looks at `frame`. Between two keys it glides, with the size changing
 * geometrically so a zoom feels even. */
export const viewAt = (keys: ViewKey[], frame: number, fallback: Rect): Rect => {
  if (keys.length === 0) return fallback;
  const sorted = [...keys].sort((a, b) => a.at - b.at);
  let i = sorted.findIndex((k) => k.at > frame);
  if (i === -1) i = sorted.length;
  if (i === 0) return sorted[0].view;
  const to = sorted[i - 1];
  const frames = to.frames ?? 18;
  if (to.move === 'cut' || frames <= 0 || i < 2) return to.view;
  // A glide starts from where the camera is when its key arrives: mid-way through the glide
  // before, when that one has not finished.
  const from = viewAt(sorted.slice(0, i - 1), to.at, fallback);
  const t = interpolate(frame, [to.at, to.at + frames], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeInOut });
  const geo = (a: number, b: number) => a * Math.pow(b / a, t);
  const lin = (a: number, b: number) => a + (b - a) * t;
  const w = geo(from.w, to.view.w);
  const h = geo(from.h, to.view.h);
  const cx = lin(from.x + from.w / 2, to.view.x + to.view.w / 2);
  const cy = lin(from.y + from.h / 2, to.view.y + to.view.h / 2);
  return { x: cx - w / 2, y: cy - h / 2, w, h };
};

/** How many screen pixels one source pixel is, inside a Camera: a mark drawn in source pixels
 * divides its line widths and type by it to keep the same size on screen while the camera zooms. */
const ScaleContext = createContext(1);
export const useCameraScale = (): number => useContext(ScaleContext);

/** A source video seen through a camera that fills a `width` × `height` box (default the
 * composition): a fixed `view`, or `keys` it glides between. `children` are drawn over the footage
 * in the source's pixels, so a mark stays on what it marks while the camera moves. */
export const Camera: React.FC<{
  src: string;
  source: Size;
  view?: Rect;
  keys?: ViewKey[];
  /** Extra zoom about the view's centre (a punch-in); default 1. */
  zoom?: number;
  width?: number;
  height?: number;
  /** Frames of the source to skip (Remotion's trimBefore). */
  trimBefore?: number;
  muted?: boolean;
  volume?: number;
  id?: string;
  style?: React.CSSProperties;
  children?: React.ReactNode;
}> = ({ src, source, view, keys, zoom = 1, width, height, trimBefore, muted, volume, id, style, children }) => {
  const frame = useCurrentFrame();
  const config = useVideoConfig();
  const box = { width: width ?? config.width, height: height ?? config.height };
  const looking = keys ? viewAt(keys, frame, view ?? whole(source)) : view ?? whole(source);
  const v = fitView(looking, box, source, zoom);
  const s = box.width / v.w;
  return (
    <div data-id={id} style={{ position: 'relative', width: box.width, height: box.height, overflow: 'hidden', background: '#000', ...style }}>
      <div style={{
        position: 'absolute', left: 0, top: 0, width: source.width, height: source.height, transformOrigin: '0 0',
        transform: `translate(${-v.x * s}px, ${-v.y * s}px) scale(${s})`,
      }}>
        <OffthreadVideo src={src} trimBefore={trimBefore} muted={muted} volume={volume} style={{ width: source.width, height: source.height }} />
        <ScaleContext.Provider value={s}>{children}</ScaleContext.Provider>
      </div>
    </div>
  );
};

// ---- Cutting out dead air ----

export type ClipSegment = { from: number; frames: number; trimBefore: number; startMs: number; endMs: number };

/** The kept spans as segments of the composition: where each starts (`from`), how long it runs
 * and where in the source it plays from. Built from the running total, so rounding never drifts. */
export const cutSegments = (keep: Keep, fps: number): ClipSegment[] => {
  let total = 0;
  return keep.map(([startMs, endMs]) => {
    const from = Math.round((total / 1000) * fps);
    total += Math.max(0, endMs - startMs);
    const frames = Math.round((total / 1000) * fps) - from;
    return { from, frames, trimBefore: Math.round((startMs / 1000) * fps), startMs, endMs };
  }).filter((s) => s.frames > 0);
};

/** Frames the kept spans last: the composition's `durationInFrames`. */
export const keptFrames = (keep: Keep, fps: number): number => {
  const total = keep.reduce((sum, [a, b]) => sum + Math.max(0, b - a), 0);
  return Math.round((total / 1000) * fps);
};

/** A moment of the source (ms) in the edited clip (ms), or null when it was cut. */
export const editedMs = (ms: number, keep: Keep): number | null => {
  let before = 0;
  for (const [a, b] of keep) {
    if (ms >= a && ms <= b) return before + (ms - a);
    before += Math.max(0, b - a);
  }
  return null;
};

/** The words of a source transcript as the edited clip says them. A word in a cut is dropped. A
 * word that runs over a cut's edge is trimmed to it. */
export const remapCaptions = (captions: Caption[], keep: Keep): Caption[] => {
  const out: Caption[] = [];
  for (const c of captions) {
    for (const [a, b] of keep) {
      const start = Math.max(c.startMs, a);
      const end = Math.min(c.endMs, b);
      if (end - start < Math.min(80, (c.endMs - c.startMs) / 2)) continue;
      const s = editedMs(start, keep)!;
      out.push({ ...c, startMs: s, endMs: s + (end - start), timestampMs: c.timestampMs == null ? null : s + (end - start) / 2 });
      break;
    }
  }
  return out;
};

/** Kept spans from a transcript's words: the speech from `startMs` to `endMs`, with every pause
 * longer than `maxGapMs` (default 650) cut down to `padMs` (default 120) each side. */
export const keepSpeech = (captions: Caption[], startMs: number, endMs: number, maxGapMs = 650, padMs = 120): Keep => {
  const words = captions.filter((c) => c.endMs > startMs && c.startMs < endMs).sort((a, b) => a.startMs - b.startMs);
  if (words.length === 0) return [[startMs, endMs]];
  const keep: Keep = [];
  let a = Math.max(startMs, words[0].startMs - padMs);
  let last = words[0].endMs;
  for (const w of words.slice(1)) {
    if (w.startMs - last > maxGapMs) {
      keep.push([a, last + padMs]);
      a = w.startMs - padMs;
    }
    last = Math.max(last, w.endMs);
  }
  keep.push([a, Math.min(endMs, last + padMs * 3)]);
  return keep;
};

// ---- Punch-ins ----

/** A punch-in: from frame `at`, zoom to `scale` (default 1.18) for `frames` (default 24). */
export type Punch = { at: number; frames?: number; scale?: number };

/** The zoom the punches ask for at this frame: in fast (4 frames), held, out slower (10). */
export const usePunch = (punches: Punch[] = []): number => {
  const frame = useCurrentFrame();
  let zoom = 1;
  for (const p of punches) {
    const scale = p.scale ?? 1.18;
    const end = p.at + (p.frames ?? 24);
    const inP = interpolate(frame, [p.at, p.at + 4], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeOut });
    const outP = interpolate(frame, [end, end + 10], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeIn });
    zoom = Math.max(zoom, 1 + (scale - 1) * Math.min(inP, outP));
  }
  return zoom;
};

// ---- A stream's vertical layout ----

/** A stream made vertical. `split` puts the facecam (`cam`) over the gameplay (`game`), the cam
 * taking `camShare` of the height (default 0.36). `game` and `cam` fill the frame alone. `keep`
 * plays only those spans of the source, in order (dead air cut). `punches` zoom the panel they
 * name (default the cam) at frames of the edited clip. `children` are drawn over it all:
 * captions, the hook, marks. */
export const StreamClip: React.FC<{
  src: string;
  source: Size;
  cam?: Rect | null;
  game?: Rect;
  layout?: 'split' | 'game' | 'cam';
  camShare?: number;
  keep?: Keep;
  punches?: (Punch & { panel?: 'cam' | 'game' })[];
  volume?: number;
  id?: string;
  children?: React.ReactNode;
}> = ({ src, source, cam, game, layout, camShare = 0.36, keep, punches = [], volume, id, children }) => {
  const { width, height, fps, durationInFrames } = useVideoConfig();
  const mode = layout ?? (cam ? 'split' : 'game');
  const camPunch = usePunch(punches.filter((p) => (p.panel ?? 'cam') === 'cam'));
  const gamePunch = usePunch(punches.filter((p) => p.panel === 'game'));
  const segments = keep ? cutSegments(keep, fps) : [{ from: 0, frames: durationInFrames, trimBefore: 0, startMs: 0, endMs: 0 }];
  const camH = Math.round(height * camShare);
  const panels = (trimBefore: number) => {
    if (mode === 'cam' && cam) {
      return <Camera src={src} source={source} view={cam} zoom={camPunch} trimBefore={trimBefore} volume={volume} id="cam" />;
    }
    if (mode === 'game' || !cam) {
      return <Camera src={src} source={source} view={game ?? whole(source)} zoom={gamePunch} trimBefore={trimBefore} volume={volume} id="game" />;
    }
    return (
      <div style={{ display: 'flex', flexDirection: 'column', width, height }}>
        <Camera src={src} source={source} view={cam} zoom={camPunch} width={width} height={camH} trimBefore={trimBefore} muted id="cam" />
        <Camera src={src} source={source} view={game ?? whole(source)} zoom={gamePunch} width={width} height={height - camH} trimBefore={trimBefore} volume={volume} id="game" />
      </div>
    );
  };
  return (
    <AbsoluteFill data-id={id} style={{ background: '#000' }}>
      {segments.map((s, i) => (
        <Sequence key={i} from={s.from} durationInFrames={s.frames} premountFor={Math.round(fps / 2)}>
          {panels(s.trimBefore)}
        </Sequence>
      ))}
      <AbsoluteFill>{children}</AbsoluteFill>
    </AbsoluteFill>
  );
};

// ---- Marks for guides ----

/** A ring drawn around `rect` from frame `at`, with an optional `label` beside it (`side`). Inside
 * a Camera, `rect` is in the source's pixels and the ring keeps its size on screen as the camera
 * zooms. It leaves at `until` when that is given. */
export const Callout: React.FC<{
  rect: Rect;
  at?: number;
  until?: number;
  label?: string;
  side?: 'top' | 'bottom' | 'left' | 'right';
  color?: string;
  /** Line width in screen px; default 8. */
  line?: number;
  /** Label size in screen px; default 48. */
  size?: number;
  radius?: number;
  id?: string;
}> = ({ rect, at = 0, until, label, side = 'bottom', color = theme.colors.accent, line = 8, size = 48, radius = 18, id }) => {
  const frame = useCurrentFrame();
  const s = useCameraScale();
  const draw = useProgress({ delay: at, duration: 14 });
  const tag = useSpring(at + 8);
  const leave = useExit(until ?? Number.MAX_SAFE_INTEGER, 8);
  if (frame < at || leave <= 0) return null;
  const lw = line / s;
  const pad = 10 / s;
  const w = rect.w + pad * 2;
  const h = rect.h + pad * 2;
  const perimeter = 2 * (w + h);
  const fontSize = size / s;
  const gap = 14 / s;
  const place: React.CSSProperties = side === 'bottom' ? { left: rect.x + rect.w / 2, top: rect.y + rect.h + pad + gap, transform: 'translateX(-50%)' }
    : side === 'top' ? { left: rect.x + rect.w / 2, top: rect.y - pad - gap, transform: 'translate(-50%, -100%)' }
    : side === 'right' ? { left: rect.x + rect.w + pad + gap, top: rect.y + rect.h / 2, transform: 'translateY(-50%)' }
    : { left: rect.x - pad - gap, top: rect.y + rect.h / 2, transform: 'translate(-100%, -50%)' };
  return (
    <div data-id={id} style={{ position: 'absolute', left: 0, top: 0, opacity: leave }}>
      <svg style={{ position: 'absolute', left: rect.x - pad - lw, top: rect.y - pad - lw, overflow: 'visible' }} width={w + lw * 2} height={h + lw * 2}>
        <rect x={lw} y={lw} width={w} height={h} rx={radius / s} fill="none" stroke={color} strokeWidth={lw}
          strokeDasharray={perimeter} strokeDashoffset={perimeter * (1 - draw)} strokeLinecap="round" />
      </svg>
      {label ? (
        <div style={{ position: 'absolute', ...place }}>
          <div style={{
            ...pop(tag), whiteSpace: 'nowrap', fontFamily: theme.fonts.display, fontWeight: 800, fontSize, color: '#0B0D12',
            background: color, padding: `${fontSize * 0.22}px ${fontSize * 0.45}px`, borderRadius: fontSize * 0.3,
          }}>{label}</div>
        </div>
      ) : null}
    </div>
  );
};

/** An arrow drawn from `from` to `to` from frame `at`, bowed by `bend` (a fraction of its length;
 * default 0.2, negative bows the other way). Same coordinates as Callout. */
export const Arrow: React.FC<{
  from: { x: number; y: number };
  to: { x: number; y: number };
  at?: number;
  until?: number;
  bend?: number;
  color?: string;
  line?: number;
  id?: string;
}> = ({ from, to, at = 0, until, bend = 0.2, color = theme.colors.accent, line = 8, id }) => {
  const frame = useCurrentFrame();
  const s = useCameraScale();
  const draw = useProgress({ delay: at, duration: 16 });
  const leave = useExit(until ?? Number.MAX_SAFE_INTEGER, 8);
  if (frame < at || leave <= 0) return null;
  const lw = line / s;
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const cx = (from.x + to.x) / 2 - dy * bend;
  const cy = (from.y + to.y) / 2 + dx * bend;
  // The head points along the curve's last tangent, from the control point to the tip.
  const angle = Math.atan2(to.y - cy, to.x - cx);
  const head = 28 / s;
  const wing = (turn: number) => `${to.x - head * Math.cos(angle + turn)},${to.y - head * Math.sin(angle + turn)}`;
  return (
    <svg data-id={id} style={{ position: 'absolute', left: 0, top: 0, overflow: 'visible', opacity: leave }} width={1} height={1}>
      <path d={`M ${from.x} ${from.y} Q ${cx} ${cy} ${to.x} ${to.y}`} fill="none" stroke={color} strokeWidth={lw} strokeLinecap="round"
        pathLength={1} strokeDasharray={1} strokeDashoffset={1 - draw} />
      {draw > 0.85 ? <polyline points={`${wing(0.5)} ${to.x},${to.y} ${wing(-0.5)}`} fill="none" stroke={color} strokeWidth={lw} strokeLinecap="round" strokeLinejoin="round" /> : null}
    </svg>
  );
};

/** A numbered step across the top of the safe area: "2  Open the Anvil". It is placed in the
 * composition's pixels: put it beside a Camera, not inside one. */
export const Step: React.FC<{ n: number | string; text: string; at?: number; until?: number; color?: string; size?: number; id?: string }> = ({
  n, text, at = 0, until, color = theme.colors.accent, size = 52, id,
}) => {
  const frame = useCurrentFrame();
  const safe = useSafeArea();
  const p = useSpring(at);
  const leave = useExit(until ?? Number.MAX_SAFE_INTEGER, 8);
  if (frame < at || leave <= 0) return null;
  return (
    <div style={{ position: 'absolute', top: safe.top, left: safe.left, right: safe.right, display: 'flex', justifyContent: 'center', opacity: leave }}>
      <div data-id={id} style={{
        ...pop(p, 0.9), display: 'flex', alignItems: 'center', gap: size * 0.4, padding: `${size * 0.25}px ${size * 0.5}px ${size * 0.25}px ${size * 0.25}px`,
        background: 'rgba(11,13,18,0.86)', borderRadius: size * 0.5, fontFamily: theme.fonts.display, fontWeight: 800, fontSize: size, color: '#F5F7FA',
      }}>
        <span style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center', width: size * 1.3, height: size * 1.3, borderRadius: '50%', background: color, color: '#0B0D12' }}>{n}</span>
        <span>{text}</span>
      </div>
    </div>
  );
};

/** The opening line, at the top of the safe area for `frames` (default 2.6 s): `card` is dark
 * words on a light card, `plain` light words with a shadow. */
export const Hook: React.FC<{ text: string; at?: number; frames?: number; variant?: 'card' | 'plain'; size?: number; id?: string }> = ({
  text, at = 0, frames, variant = 'card', size = 64, id,
}) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const safe = useSafeArea();
  const end = at + (frames ?? Math.round(fps * 2.6));
  // Already in at its first frame: that frame is the thumbnail.
  const spring = useSpring(at, 16);
  const p = at === 0 ? 1 : spring;
  const leave = useExit(end, 8);
  if (frame < at || leave <= 0) return null;
  const card = variant === 'card';
  return (
    <div style={{ position: 'absolute', top: safe.top, left: safe.left, right: safe.right, display: 'flex', justifyContent: 'center', opacity: leave }}>
      <div data-id={id} style={{
        ...pop(p, 0.8), maxWidth: '100%', textAlign: 'center', textWrap: 'balance', fontFamily: theme.fonts.display, fontWeight: 800, fontSize: size, lineHeight: 1.12,
        color: card ? '#0B0D12' : '#FFFFFF', background: card ? '#FFFFFF' : undefined, padding: card ? `${size * 0.28}px ${size * 0.5}px` : undefined,
        borderRadius: card ? size * 0.28 : undefined, textShadow: card ? undefined : '0 4px 18px rgba(0,0,0,0.6)',
      }}>{text}</div>
    </div>
  );
};
