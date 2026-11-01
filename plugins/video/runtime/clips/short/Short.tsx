// Written by Relay's Clips space (plugins/video/runtime/clips/short/): every clip of a VOD is this
// one composition, drawn from its edit as props. Edits here are overwritten; change a clip in the
// Clips space, or its props.
//
// The look comes from the person's first review of hand-made clips: cuts dissolve over 5 frames
// (picture and sound), one crop per clip with a reframe only where the person would leave it, at
// most one punch-in, a chunky pixel face with a thick outline and a hard shadow, the opening line
// across the top, and a big end card over the dimmed last frame.
import type { Caption } from '@remotion/captions';
import { fitText } from '@remotion/layout-utils';
import React from 'react';
import { AbsoluteFill, interpolate, Sequence, staticFile, useCurrentFrame, useVideoConfig, type CalculateMetadataFunction } from 'remotion';
import { z } from 'zod';
import {
  Camera, Captions, crossSegments, CutFade, cutLevel, easeInOut, easeOut, editedMs, keepSpeech, loadFonts, remapCaptions, useSafeArea, useSpring,
  type Keep, type Rect,
} from '@relay/video';

loadFonts({ family: 'Pixelify Sans', file: 'assets/PixelifySans.ttf', weight: '400 700' });

const PIXEL = '"Pixelify Sans"';
const SANS = 'Inter';
/** Frames each cut dissolves over, picture and sound. */
const CROSSFADE = 5;
/** Seconds kept after the last word, under the end card; without one, a short beat. */
const TAIL = 1.6;
const BEAT = 0.4;
/** Seconds the end card is on screen. */
const END_CARD = 2.6;
/** Seconds the opening line is on screen. */
const HOOK = 2.4;
const HARD_SHADOW = '0 7px 0 #000, 0 0 30px rgba(0,0,0,0.45)';
const PURPLE = '#C9A7FF';

const rect = z.object({ x: z.number(), y: z.number(), w: z.number(), h: z.number() });

export const schema = z.object({
  hook: z.string().describe('Opening line'),
  captions: z.enum(['pixel', 'bold', 'clean', 'none']).describe('Captions'),
  highlight: z.string().describe('Key word colour'),
  captionY: z.number().nullable().describe('Captions height (0–1)'),
  endCard: z.boolean().describe('End card'),
  layout: z.enum(['split', 'fit', 'crop', 'cam', 'game']).describe('Framing'),
  src: z.string(),
  fps: z.number().optional(),
  source: z.object({ width: z.number(), height: z.number() }),
  words: z.array(z.any()),
  keep: z.array(z.tuple([z.number(), z.number()])),
  tightenMs: z.number().nullable(),
  view: rect.nullable(),
  reframe: z.record(z.string(), rect),
  cam: rect.nullable(),
  game: rect,
  emphasis: z.array(z.string()),
  fixes: z.record(z.string(), z.string()),
  punchAt: z.array(z.number()),
  end: z.object({ top: z.string(), handle: z.string() }),
});
export type Props = z.infer<typeof schema>;

export const DEFAULT: Props = {
  hook: 'The opening line', captions: 'pixel', highlight: '#FFE14D', captionY: null, endCard: true, layout: 'fit', src: 'assets/clip.mp4', fps: 30,
  source: { width: 1920, height: 1080 }, words: [], keep: [[0, 5000]], tightenMs: null, view: null, reframe: {}, cam: null,
  game: { x: 0, y: 0, w: 1920, h: 1080 }, emphasis: [], fixes: {}, punchAt: [], end: { top: 'Live on Twitch', handle: '' },
};

/** Fix what was misheard: `{heard: meant}`, case and trailing punctuation ignored. */
const fixWords = (words: Caption[], fixes: Record<string, string>): Caption[] => {
  const keys = Object.fromEntries(Object.entries(fixes).map(([k, v]) => [k.toLowerCase(), v]));
  if (Object.keys(keys).length === 0) return words;
  return words.map((w) => {
    const bare = w.text.trim().replace(/[.,!?…]+$/, '');
    const meant = keys[bare.toLowerCase()];
    return meant ? { ...w, text: w.text.replace(bare, meant) } : w;
  });
};

/** The spans the clip plays (dead air cut inside each), which part of `keep` each came from, the
 * words on the edited timing, and the spans with the tail run on under the end. */
export const plan = (p: Props) => {
  const words = fixWords(p.words as Caption[], p.fixes ?? {});
  const parts: Keep = p.keep.map(([a, b]) => [Math.round(a), Math.round(b)]);
  const spans = parts.flatMap(([a, b], part) =>
    (p.tightenMs ? keepSpeech(words, a, b, p.tightenMs, 120) : [[a, b] as [number, number]]).map((span) => ({ span: span as [number, number], part })));
  const spoken: Keep = spans.map((s) => s.span);
  const captions = remapCaptions(words, spoken);
  const tail = (p.endCard ? TAIL : BEAT) * 1000;
  const keep: Keep = spoken.map(([a, b], i) => (i === spoken.length - 1 ? [a, b + tail] : [a, b]));
  return { keep, spoken, captions, partOf: spans.map((s) => s.part) };
};

export const metadata: CalculateMetadataFunction<Props> = ({ props }) => {
  const fps = props.fps ?? 30;
  const { keep } = plan(props);
  const ms = keep.reduce((sum, [a, b]) => sum + Math.max(0, b - a), 0);
  return { fps, durationInFrames: Math.max(fps, Math.round((ms / 1000) * fps)) };
};

const outlined = (size: number, family: string, weight = 700): React.CSSProperties => ({
  fontFamily: family, fontWeight: weight, fontSize: size, lineHeight: 1.06, textTransform: 'uppercase',
  WebkitTextStroke: `${Math.round(size * (family === PIXEL ? 0.2 : 0.12))}px #000`, paintOrder: 'stroke fill', textShadow: HARD_SHADOW,
});

const fit = (text: string, room: number, family: string, weight: number, max: number, upper = true) =>
  Math.min(max, fitText({ text, withinWidth: room, fontFamily: family, fontWeight: weight, textTransform: upper ? 'uppercase' : undefined, validateFontIsLoaded: false }).fontSize);

/** The opening line, across the top for the first seconds: the thumbnail's words. */
const Opening: React.FC<{ text: string; style: Props['captions']; top: number }> = ({ text, style, top }) => {
  const frame = useCurrentFrame();
  const { fps, width } = useVideoConfig();
  const safe = useSafeArea();
  const end = Math.round(fps * HOOK);
  if (!text.trim() || frame >= end) return null;
  const leave = interpolate(frame, [end - 8, end], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeOut });
  const room = width - safe.left - safe.right - 40;
  const longest = text.split(/\s+/).reduce((a, b) => (b.length > a.length ? b : a), '');
  if (style === 'clean') {
    const size = fit(longest, room - 60, SANS, 800, 76, false);
    return (
      <div data-id="hook" style={{ position: 'absolute', top, left: safe.left, right: safe.right, display: 'flex', justifyContent: 'center', opacity: leave }}>
        <div style={{
          fontFamily: SANS, fontWeight: 800, fontSize: size, lineHeight: 1.12, color: '#fff', textAlign: 'center', padding: '18px 30px',
          background: 'rgba(8,8,10,0.78)', borderRadius: 18, maxWidth: room,
        }}>{text}</div>
      </div>
    );
  }
  const family = style === 'bold' ? SANS : PIXEL;
  const weight = style === 'bold' ? 900 : 700;
  const size = fit(longest, room, family, weight, family === PIXEL ? 104 : 92);
  return (
    <div data-id="hook" style={{ position: 'absolute', top, left: safe.left, right: safe.right, textAlign: 'center', opacity: leave, color: '#fff', ...outlined(size, family, weight) }}>
      {text}
    </div>
  );
};

/** The end: the frame dims and the card pops in, big. */
const EndCard: React.FC<{ end: Props['end']; family: string }> = ({ end, family }) => {
  const frame = useCurrentFrame();
  const { fps, width, durationInFrames } = useVideoConfig();
  const safe = useSafeArea();
  const at = durationInFrames - Math.round(fps * END_CARD);
  const dim = interpolate(frame, [at, at + 10], [0, 0.55], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeInOut });
  const first = useSpring(at, 11);
  const button = useSpring(at + 5, 10);
  const second = useSpring(at + 12, 11);
  if (frame < at) return null;
  const weight = family === PIXEL ? 700 : 900;
  const room = width - safe.left - safe.right - 60;
  const sub = fit('Subscribe', room, family, weight, 150) * 0.86;
  const handle = end.handle ? fit(end.handle, room, family, weight, 170) : 0;
  const pop = (p: number): React.CSSProperties => ({ opacity: Math.min(1, p * 1.5), transform: `scale(${0.5 + 0.5 * p})` });
  return (
    <AbsoluteFill>
      <AbsoluteFill style={{ background: `rgba(0,0,0,${dim})` }} />
      <div data-id="endcard" style={{
        position: 'absolute', left: safe.left, right: safe.right, top: '19%', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 26, textAlign: 'center',
      }}>
        <div style={{ ...outlined(Math.round(sub * 0.95), family, weight), color: '#fff', ...pop(first) }}>Like &amp;</div>
        <div style={{
          ...outlined(Math.round(sub * 0.9), family, weight), color: '#fff', ...pop(button), WebkitTextStroke: `${Math.round(sub * 0.12)}px #000`,
          background: '#FF0033', border: '8px solid #000', borderRadius: 26, padding: '14px 40px 22px', boxShadow: '0 12px 0 #000',
        }}>Subscribe</div>
        {end.handle ? (
          <div style={{ ...pop(second), marginTop: 34 }}>
            <div style={{ ...outlined(Math.round(handle * 0.5), family, weight), color: '#FFE14D' }}>{end.top}</div>
            <div style={{ ...outlined(handle, family, weight), color: PURPLE, marginTop: 8 }}>{end.handle}</div>
          </div>
        ) : null}
      </div>
    </AbsoluteFill>
  );
};

/** The captions, in the clip's style. */
const Words: React.FC<{ captions: Caption[]; p: Props; y: number }> = ({ captions, p, y }) => {
  if (p.captions === 'none' || captions.length === 0) return null;
  if (p.captions === 'pixel') {
    return (
      <Captions id="captions" captions={captions} variant="plain" maxWords={1} y={y} uppercase size={150} font={PIXEL} weight={700}
        stroke={0.2} outline="#000" highlight={p.highlight} emphasis={p.emphasis} combineWithinMs={0} style={{ textShadow: HARD_SHADOW }} />
    );
  }
  if (p.captions === 'bold') {
    return (
      <Captions id="captions" captions={captions} variant="pop" maxWords={3} y={y} uppercase size={96} font={SANS} weight={900}
        stroke={0.12} outline="#000" highlight={p.highlight} emphasis={p.emphasis} combineWithinMs={900} style={{ textShadow: HARD_SHADOW }} />
    );
  }
  return (
    <Captions id="captions" captions={captions} variant="lit" maxWords={6} y={y} size={62} font={SANS} weight={700}
      stroke={0} highlight={p.highlight} emphasis={p.emphasis} combineWithinMs={1400}
      style={{ background: 'rgba(8,8,10,0.72)', borderRadius: 16, padding: '10px 22px', textShadow: 'none' }} />
  );
};

/** One span's picture: a crop that fills the frame, or a wider one across the middle on a
 * blurred copy of itself. */
const Shot: React.FC<{ p: Props; src: string; view: Rect; trimBefore: number; zoom: number; level: (f: number) => number; top?: number; height?: number; id: string }> = ({
  p, src, view, trimBefore, zoom, level, top, height: boxH, id,
}) => {
  const { width, height } = useVideoConfig();
  const area = boxH ?? height;
  const natural = Math.round((width * view.h) / view.w / 2) * 2;
  const h = Math.min(area, natural);
  const y = (top ?? 0) + Math.round((area - h) / 2);
  return (
    <>
      {h < area && top === undefined ? (
        <Camera src={src} source={p.source} view={view} trimBefore={trimBefore} muted zoom={1.4}
          style={{ position: 'absolute', inset: 0, filter: 'blur(36px) brightness(0.5)' }} />
      ) : null}
      <Camera id={id} src={src} source={p.source} view={view} width={width} height={h} zoom={zoom} trimBefore={trimBefore} volume={level}
        style={{ position: 'absolute', left: 0, top: y }} />
    </>
  );
};

export const Short: React.FC<Props> = (p) => {
  const { fps, width, height, durationInFrames } = useVideoConfig();
  const frame = useCurrentFrame();
  const safe = useSafeArea();
  const { keep, captions, partOf } = plan(p);
  // The footage is in the video's public/: an asset name, as the engine gives it.
  const src = /^(https?:|\/)/.test(p.src) ? p.src : staticFile(p.src);
  const segments = crossSegments(keep, fps, CROSSFADE);
  const whole: Rect = { x: 0, y: 0, w: p.source.width, h: p.source.height };
  // A punch-in eases in over a third of a second, holds while the line lands, and eases out.
  const zoom = (p.punchAt ?? []).reduce((z, ms) => {
    const at = Math.round(((editedMs(ms, keep) ?? -1e9) / 1000) * fps);
    const k = Math.min(
      interpolate(frame, [at - 4, at + 8], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeInOut }),
      interpolate(frame, [at + 40, at + 58], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeInOut }),
    );
    return Math.max(z, 1 + 0.12 * k);
  }, 1);
  const tail = Math.round(fps * (p.endCard ? TAIL : BEAT));
  const endFrom = durationInFrames - tail;
  const camShare = 0.36;
  const seam = Math.round(height * camShare);
  // Where the words sit: on a split's seam, under a picture across the middle, else low.
  const narrowFit = p.layout === 'fit' || p.layout === 'cam' || (p.view && p.view.w / p.view.h > 0.7 && (p.layout === 'crop' || p.layout === 'game'));
  const captionY = p.captionY ?? (p.layout === 'split' ? camShare : narrowFit ? 0.74 : 0.64);
  const hookTop = p.layout === 'split' ? safe.top + 10 : narrowFit ? Math.max(safe.top, height * 0.17) : safe.top + 24;
  const family = p.captions === 'bold' || p.captions === 'clean' ? SANS : PIXEL;
  return (
    <AbsoluteFill style={{ background: '#000' }}>
      {segments.map((s, i) => {
        // The sound fades out over the tail, under the end card.
        const level = (f: number) => cutLevel(s, f) * interpolate(s.from + f, [endFrom, durationInFrames - 2], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
        const own = p.reframe?.[String(partOf[i])];
        let body: React.ReactNode;
        if (p.layout === 'split' && p.cam) {
          body = (
            <>
              <Camera id="cam" src={src} source={p.source} view={p.cam} width={width} height={seam} zoom={zoom} trimBefore={s.trimBefore} muted
                style={{ position: 'absolute', left: 0, top: 0 }} />
              <Camera id="game" src={src} source={p.source} view={own ?? p.view ?? p.game} width={width} height={height - seam} trimBefore={s.trimBefore} volume={level}
                style={{ position: 'absolute', left: 0, top: seam }} />
              <div style={{ position: 'absolute', left: 0, right: 0, top: seam - 3, height: 6, background: '#000' }} />
            </>
          );
        } else {
          const view = own ?? p.view ?? (p.layout === 'cam' && p.cam ? p.cam : p.layout === 'game' ? p.game : whole);
          body = <Shot p={p} src={src} view={view} trimBefore={s.trimBefore} zoom={p.layout === 'fit' ? 1 : zoom} level={level} id="shot" />;
        }
        return (
          <Sequence key={i} from={s.from} durationInFrames={s.frames} premountFor={Math.round(fps / 2)}>
            <CutFade seg={s}>{body}</CutFade>
          </Sequence>
        );
      })}
      <Opening text={p.hook} style={p.captions} top={hookTop} />
      {p.endCard ? <EndCard end={p.end} family={family} /> : null}
      <Words captions={captions} p={p} y={captionY} />
    </AbsoluteFill>
  );
};
