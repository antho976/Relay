import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {Camera, EASE, Shot} from './Camera';
import {Clip, Device, FocusMove, Media, Seam, SeamDir} from './Device';
import {Center, Plate, Split} from './Layout';
import {Cue, CuePoint, LEAD} from './Sound';
import {Body, Counter, EraTag, Eyebrow, Line, Tag, Title, useEdgeFade, useRise, useWipe} from './Type';
import {ACCENT, EIGHTH, MONO, MUTED, ON_BG, QUARTER, SERIF, run} from './theme';

/** One thing the copy claims, and where on the capture it is. */
export type Step = {
  /** beat-local frame the claim lights up */
  at: number;
  text: string;
  /** x, y, w, h as fractions of the screen */
  box: [number, number, number, number];
};

/* ── openers ─────────────────────────────────────────────────────────────── */

export const Card: React.FC<{
  eyebrow: string; title: string; sub?: string; shot?: Shot;
  /** frame the sub-line lands on — used to put the closing line on the bed's final chord */
  subAt?: number;
  /** the sub's two figures count in rather than fading up */
  count?: [number, number];
  /** the title first reads as this, then becomes `title` on `flipAt` — "0.8.9" becoming "0.9" */
  flipFrom?: string;
  flipAt?: number;
}> = ({eyebrow, title, sub, shot, subAt = 16, count, flipFrom, flipAt = 0}) => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);
  const rise = useRise(subAt - LEAD, 10);
  const p = interpolate(frame, [6, 40], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.glide,
  });
  const fig = (n: number) => Math.round(n * p).toLocaleString('en-US');
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {sub && !count ? <Cue at={subAt} sfx="reveal" gain={0.8} /> : null}
        {flipFrom ? <Cue at={flipAt} sfx="sweep" gain={0.85} /> : null}
        <Camera shot={shot}>
          <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center'}}>
            <Center gap={26}>
              <Eyebrow delay={0}>{eyebrow}</Eyebrow>
              {flipFrom ? <VersionFlip from={flipFrom} to={title} at={flipAt} /> : <Title delay={4} size={132}>{title}</Title>}
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

/**
 * "0.8.9" becoming "0.9". The characters the two share stay put; the ones only the old version
 * has shrink to nothing between them, so the 0 and the 9 close up — the same compression the
 * onboarding beat makes, on the one line the film ends on. Not a crossfade: the middle is gone
 * before the gap has finished closing. The sweep, the sound every version change in the film
 * makes, fires on `at`.
 */
const VersionFlip: React.FC<{from: string; to: string; at: number}> = ({from, to, at}) => {
  const frame = useCurrentFrame();
  // longest common prefix / suffix, so "0.8.9" → "0.9" keeps "0." and "9"
  let i = 0;
  while (i < from.length && i < to.length && from[i] === to[i]) i++;
  let j = 0;
  while (j < from.length - i && j < to.length - i && from[from.length - 1 - j] === to[to.length - 1 - j]) j++;
  const head = from.slice(0, i);
  const mid = from.slice(i, from.length - j);
  const tail = from.slice(from.length - j);
  const p = interpolate(frame, [at, at + 16], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.glide,
  });
  const gone = interpolate(p, [0, 0.55], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const w = useWipe(4);
  return (
    <div style={{fontFamily: SERIF, fontSize: 132, lineHeight: 1.06, color: ON_BG, whiteSpace: 'nowrap', ...w.outer}}>
      <span style={{display: 'inline-block', ...w.inner}}>
        <span>{head}</span>
        {/* Same colour and size as its neighbours until the flip: it has to read as one word.
            Plain inline, no overflow clipping — an inline-block with overflow:hidden sits its
            bottom edge on the baseline and lifts the glyph. */}
        <span style={{fontSize: `${Math.max(0.01, 1 - p) * 100}%`, opacity: gone}}>{mid}</span>
        <span>{tail}</span>
      </span>
    </div>
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
  /** something visible happening on the capture after the seam — a toggle, a view switching */
  cues?: CuePoint[];
}> = ({
  before, after, eyebrow, title, note, line, beforeSpan, afterSpan,
  hold, sweep = 26, dir = 'ltr', height = 830, flip = false, focus, shot, cues,
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
        {/* The sweep is the only sound the seam makes. It used to be followed by a second thunk
            when the edge reached the far side, and with five seams in the film that pair became
            the most repeated sound in it. */}
        <Cue at={start} sfx="sweep" />
        {(cues ?? []).map((c, i) => <Cue key={i} {...c} />)}
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
  /** something visible happening on the capture: a set logged, a view switching. Not the beat
   *  starting — the old `changes={[8]}` fired a thunk eight frames into the push, where the only
   *  thing changing was the transition. */
  cues?: CuePoint[];
  /**
   * The copy, claim by claim, each pointing at the thing on the screen it is about. The sentence
   * lights up as its callout draws on the capture, so "it tells you what to lift" is shown rather
   * than asserted next to a screenshot. `stepsOut` is the frame the callouts leave (before the
   * capture changes underneath them).
   */
  steps?: Step[];
  stepsOut?: number;
}> = ({clip, eyebrow, title, note, line, height = 880, flip = false, focus, shot, tag, cues, steps, stepsOut}) => {
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {(cues ?? []).map((c, i) => <Cue key={i} {...c} />)}
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
                  {steps ? <StepLine steps={steps} out={stepsOut ?? durationInFrames} /> : null}
                  {tag ? <div style={{marginTop: 8}}><Tag label={tag} accent delay={16} /></div> : null}
                </>
              }
            >
              <Device
                height={height} clip={clip} focus={focus}
                overlay={steps ? steps.map((s, i) => <Callout key={i} step={s} out={stepsOut ?? durationInFrames} />) : null}
              />
            </Split>
          </AbsoluteFill>
        </Camera>
      </Plate>
    </AbsoluteFill>
  );
};

/** The claims as one paragraph, the current one lit. */
const StepLine: React.FC<{steps: Step[]; out: number}> = ({steps, out}) => {
  const frame = useCurrentFrame();
  const rise = useRise(11, 12);
  return (
    <div style={{fontFamily: SERIF, fontSize: 30, lineHeight: 1.35, color: MUTED, maxWidth: 560, ...rise}}>
      {steps.map((s, i) => {
        const from = s.at - LEAD;
        const until = i + 1 < steps.length ? steps[i + 1].at - LEAD : out;
        const lit = interpolate(frame, [from, from + 6, until, until + 6], [0, 1, 1, 0], {
          extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
        });
        return (
          <span key={i} style={{color: lit > 0.5 ? ON_BG : MUTED, opacity: 0.85 + lit * 0.15, transition: 'none'}}>
            {s.text}{i + 1 < steps.length ? ' ' : ''}
          </span>
        );
      })}
    </div>
  );
};

/**
 * A callout on the capture: an accent outline around the thing the sentence is about. Drawn in
 * the screen's own coordinates so it stays on its element at any phone size. It arrives LEAD
 * frames before its sentence lights up and leaves, all of them together, before the capture
 * changes underneath.
 */
const Callout: React.FC<{step: Step; out: number}> = ({step, out}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const s = spring({frame: frame - (step.at - LEAD), fps, config: {damping: 200, stiffness: 150}});
  const leave = interpolate(frame, [out - 7, out - 1], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  if (s <= 0.001 || leave >= 1) return null;
  const [x, y, w, h] = step.box;
  return (
    <div
      style={{
        position: 'absolute', left: `${x * 100}%`, top: `${y * 100}%`, width: `${w * 100}%`, height: `${h * 100}%`,
        border: `2px solid ${ACCENT}`, borderRadius: 8, boxSizing: 'border-box',
        background: `${ACCENT}14`, boxShadow: `0 0 0 4px ${ACCENT}1A, 0 0 22px ${ACCENT}55`,
        opacity: s * (1 - leave),
        transform: `scale(${1.08 - s * 0.08 + leave * 0.05})`,
      }}
    />
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
  // One per eighth note. Each row is standing by the time its tap sounds, so the list settles in
  // time rather than a sixth of a second behind its own sound.
  const beats = run(gridStart + QUARTER * 2, items.length, EIGHTH).map((f) => f - gridStart);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {beats.map((f, i) => <Cue key={i} at={f} sfx="tap" gain={0.5 * Math.pow(0.96, i)} />)}
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
  const r = useRise(at - LEAD, 10);
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
