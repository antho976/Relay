import React from 'react';
import {AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig} from 'remotion';
import {Camera, EASE, Shot} from './Camera';
import {Clip, Device, FocusMove, Media, Seam, SeamDir} from './Device';
import {Center, Plate, Split} from './Layout';
import {Cue, CueRun} from './Sound';
import {Body, Counter, EraTag, Eyebrow, Line, Tag, Title, useEdgeFade, useRise} from './Type';
import {ACCENT, EIGHTH, MONO, MUTED, ON_BG, QUARTER, SERIF, run} from './theme';

/* ── openers ─────────────────────────────────────────────────────────────── */

export const Card: React.FC<{
  eyebrow: string; title: string; sub?: string; shot?: Shot;
  /** frame the sub-line lands on — used to put the closing line on the bed's final chord */
  subAt?: number;
  /** the sub's two figures count in rather than fading up */
  count?: [number, number];
}> = ({eyebrow, title, sub, shot, subAt = 16, count}) => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);
  const rise = useRise(subAt, 10);
  const p = interpolate(frame, [6, 40], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.glide,
  });
  const fig = (n: number) => Math.round(n * p).toLocaleString('en-US');
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {sub && !count ? <Cue at={subAt} sfx="reveal" gain={0.8} /> : null}
        <Camera shot={shot}>
          <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center'}}>
            <Center gap={26}>
              <Eyebrow delay={0}>{eyebrow}</Eyebrow>
              <Title delay={4} size={132}>{title}</Title>
              {count ? (
                <div style={{fontFamily: MONO, fontSize: 23, letterSpacing: 4, color: MUTED, fontVariantNumeric: 'tabular-nums'}}>
                  {`${fig(count[0])} commits · ${fig(count[1])} files`}
                </div>
              ) : sub ? (
                <div style={{fontFamily: MONO, fontSize: 23, letterSpacing: 4, color: MUTED, ...rise}}>{sub}</div>
              ) : null}
            </Center>
          </AbsoluteFill>
        </Camera>
      </Plate>
    </AbsoluteFill>
  );
};

/* ── the signature: one device, two eras ─────────────────────────────────── */

/**
 * The brief was "no two renders — one cool transition between the two", and it is the right call.
 * Side by side, each phone is half the size and the viewer's job becomes spot-the-difference; here
 * the same device simply *becomes* the new version under an accent edge, so the change happens to
 * something rather than being illustrated beside it.
 *
 * Both captures play live through the sweep, and where a pair is retimed BOTH sides get the same
 * span so neither runs in slow motion against the other.
 */
export const Compare: React.FC<{
  before: Clip; after: Clip;
  eyebrow: string; title: string; note?: string; line?: string;
  /** frames of real motion in each source, so two unequal scrolls finish together */
  beforeSpan?: number; afterSpan?: number;
  hold?: number; sweep?: number; dir?: SeamDir;
  height?: number; flip?: boolean; focus?: FocusMove; shot?: Shot;
}> = ({
  before, after, eyebrow, title, note, line, beforeSpan, afterSpan,
  hold, sweep = 26, dir = 'ltr', height = 830, flip = false, focus, shot,
}) => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);

  // Sit on 0.8.9 long enough to read it, sweep, then let 0.9 run out the beat.
  const start = hold ?? Math.round(durationInFrames * 0.3);
  const p = interpolate(frame, [start, start + sweep], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.sweep,
  });

  const rate = (span?: number) => (span && span > 0 ? Math.max(0.25, Math.min(3, span / durationInFrames)) : 1);

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <Cue at={start} sfx="sweep" />
        <Cue at={start + sweep} sfx="screen" />
        <Camera shot={shot}>
          <AbsoluteFill>
            <Split
              flip={flip}
              copy={
                <>
                  <Eyebrow delay={0}>{eyebrow}</Eyebrow>
                  <Title delay={4} size={66}>{title}</Title>
                  {note ? <Body delay={11}>{note}</Body> : null}
                  {line ? <Line delay={11}>{line}</Line> : null}
                </>
              }
            >
              <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 26}}>
                <Device height={height} focus={focus} glow={p > 0.02 && p < 0.98}>
                  <Seam
                    p={p}
                    dir={dir}
                    a={<Media clip={{...before, rate: before.rate ?? rate(beforeSpan)}} />}
                    b={
                      <div
                        style={{
                          position: 'absolute', inset: 0,
                          transform: `scale(${1 + (1 - Math.min(1, p * 1.35)) * 0.035})`,
                        }}
                      >
                        <Media clip={{...after, rate: after.rate ?? rate(afterSpan)}} />
                      </div>
                    }
                  />
                </Device>
                <EraTag p={p} delay={8} />
              </div>
            </Split>
          </AbsoluteFill>
        </Camera>
      </Plate>
    </AbsoluteFill>
  );
};

/* ── one capture ─────────────────────────────────────────────────────────── */

/** A single capture, with the camera flying a window over it. For things that have no before. */
export const Solo: React.FC<{
  clip: Clip; eyebrow: string; title: string; note?: string; line?: string;
  height?: number; flip?: boolean; focus?: FocusMove; shot?: Shot; tag?: string;
  /** frames, beat-relative, where the capture visibly changes screen */
  changes?: number[];
  /** frame where something on the capture fills or climbs */
  fill?: number;
}> = ({clip, eyebrow, title, note, line, height = 880, flip = false, focus, shot, tag, changes, fill}) => {
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {(changes ?? []).map((f, i) => <Cue key={i} at={f} sfx="screen" />)}
        {fill !== undefined ? <Cue at={fill} sfx="fill" /> : null}
        <Camera shot={shot}>
          <AbsoluteFill>
            <Split
              flip={flip}
              copy={
                <>
                  <Eyebrow delay={0}>{eyebrow}</Eyebrow>
                  <Title delay={4} size={72}>{title}</Title>
                  {note ? <Body delay={11}>{note}</Body> : null}
                  {line ? <Line delay={11}>{line}</Line> : null}
                  {tag ? <div style={{marginTop: 8}}><Tag label={tag} accent delay={16} /></div> : null}
                </>
              }
            >
              <Device height={height} clip={clip} focus={focus} />
            </Split>
          </AbsoluteFill>
        </Camera>
      </Plate>
    </AbsoluteFill>
  );
};

/* ── cards ───────────────────────────────────────────────────────────────── */

/**
 * The small things, named. Some changes are real but too small to shoot — a chip's proportions, a
 * renamed section, a filter — and a list is more honest than zooming on a detail nobody can read.
 */
export const ListCard: React.FC<{
  eyebrow: string; title: string; items: string[]; gridStart?: number;
}> = ({eyebrow, title, items, gridStart = 0}) => {
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 12);
  // One per eighth note. The rise and the tick share a frame, so the list settles in time.
  const beats = run(gridStart + QUARTER * 2, items.length, EIGHTH).map((f) => f - gridStart);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {beats.map((f, i) => <Cue key={i} at={f} sfx="count" gain={0.95 * Math.pow(0.96, i)} />)}
        <div style={{display: 'flex', flexDirection: 'column', gap: 32, width: 1500}}>
          <Eyebrow delay={0}>{eyebrow}</Eyebrow>
          <Title delay={4} size={76}>{title}</Title>
          <div style={{display: 'grid', gridTemplateColumns: '1fr 1fr', columnGap: 92, rowGap: 30, marginTop: 14}}>
            {items.map((it, i) => (
              <ListItem key={i} n={i} v={it} at={beats[i]} />
            ))}
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

const ListItem: React.FC<{n: number; v: string; at: number}> = ({n, v, at}) => {
  const r = useRise(at, 10);
  return (
    <div
      style={{
        display: 'flex', gap: 16, alignItems: 'baseline',
        fontFamily: SERIF, fontSize: 31, color: ON_BG, lineHeight: 1.36, ...r,
      }}
    >
      <span style={{fontFamily: MONO, fontSize: 18, color: ACCENT}}>{String(n + 1).padStart(2, '0')}</span>
      <span>{v}</span>
    </div>
  );
};

/** A figure, counted. Kept for numbers the repo can actually substantiate. */
export const NumberCard: React.FC<{
  eyebrow: string; to: number; unit: string; sub?: string; from?: number;
}> = ({eyebrow, to, unit, sub, from = 0}) => {
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 12);
  const rise = useRise(30, 10);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <Center gap={22}>
          <Eyebrow delay={0}>{eyebrow}</Eyebrow>
          <Counter to={to} from={from} start={6} span={44} size={196} />
          <div style={{fontFamily: MONO, fontSize: 25, letterSpacing: 6, color: ACCENT}}>{unit}</div>
          {sub ? (
            <div style={{fontFamily: SERIF, fontStyle: 'italic', fontSize: 28, color: MUTED, marginTop: 10, ...rise}}>
              {sub}
            </div>
          ) : null}
        </Center>
      </Plate>
    </AbsoluteFill>
  );
};
