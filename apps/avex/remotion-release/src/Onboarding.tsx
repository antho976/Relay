import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {EASE} from './Camera';
import {Plate} from './Layout';
import {Cue, LEAD} from './Sound';
import {Eyebrow, Line, Title, useEdgeFade, useRise} from './Type';
import {ACCENT, ACCENT_OLD, BG, EIGHTH, MONO, MUTED, ON_BG, OUTLINE, QUARTER, SERIF, SIXTEENTH, SURFACE_V, run, snap} from './theme';

/**
 * The onboarding beat. The brief: lay the whole old flow down page by page, then one hard hit, and
 * the new flow is standing there instead. That is the only honest way to show this change — the
 * point is not any individual page, it is *how many of them there were*, which you can only feel
 * when you watch them all arrive and then watch most of them go.
 *
 * The pages are drawn, not photographed. A 0.8.9 build no longer exists to capture, and at
 * seventeen-up a real screenshot is an illegible grey rectangle anyway; a miniature that keeps the
 * page's real heading and the shape of its control carries the information a thumbnail cannot.
 */

export type Shape =
  | 'text-field' | 'slider' | 'days' | 'summary' | 'meter'
  | `cards-${number}` | `chips-${number}` | `toggle-${number}` | `list-${number}`;

export type Page = {
  name: string;
  heading: string;
  shape: Shape;
  /** 0.8.9 only: the mono chapter eyebrow, deleted 2026-08-22. */
  eyebrow?: string;
  /** 0.9 only: the PlanLedger bar meter that sits under the question from the day-count on. */
  ledger?: boolean;
};

/* ── one page, in miniature ─────────────────────────────────────────────── */

/**
 * Each page is drawn as a small phone, not as a card. The first version used landscape wireframe
 * tiles and they read as a Figma export sitting in the middle of a film made of real screens — the
 * problem was never the size, it was the shape and the missing chrome. A portrait body with a status
 * bar, a step rail and a real CTA reads as the product at any scale, because those three things are
 * what a setup page looks like from across a room.
 *
 * Everything below is expressed as a fraction of the card's width, so one number resizes the page.
 */

const SHAPE_AR = 2.05;

const ShapeArt: React.FC<{shape: Shape; accent: string; u: number}> = ({shape, accent, u}) => {
  const line = (w: string | number, h = 0.045) => (
    <div style={{width: w, height: u * h, background: OUTLINE, borderRadius: u * 0.02}} />
  );
  if (shape === 'text-field') {
    return (
      <div
        style={{
          height: u * 0.17, marginTop: 'auto', marginBottom: 'auto',
          borderRadius: u * 0.035, border: `1px solid ${OUTLINE}`,
          background: SURFACE_V, display: 'flex', alignItems: 'center', paddingLeft: u * 0.06,
        }}
      >
        <div style={{width: Math.max(1, u * 0.012), height: u * 0.08, background: accent}} />
      </div>
    );
  }
  if (shape === 'slider') {
    return (
      <div style={{display: 'flex', flexDirection: 'column', gap: u * 0.07, height: '100%', justifyContent: 'center'}}>
        <div style={{height: u * 0.028, borderRadius: 999, background: OUTLINE, position: 'relative'}}>
          <div style={{position: 'absolute', inset: 0, width: '58%', background: accent, borderRadius: 999}} />
          <div
            style={{
              position: 'absolute', left: '58%', top: '50%', width: u * 0.09, height: u * 0.09,
              marginLeft: -u * 0.045, marginTop: -u * 0.045, borderRadius: 999, background: accent,
            }}
          />
        </div>
        {line('34%')}
      </div>
    );
  }
  if (shape === 'days') {
    return (
      <div style={{display: 'flex', gap: u * 0.022, justifyContent: 'space-between', alignItems: 'center', height: '100%'}}>
        {Array.from({length: 7}, (_, i) => {
          const on = [1, 3, 5].includes(i);
          return (
            <div
              key={i}
              style={{
                flex: 1, height: u * 0.14, borderRadius: u * 0.035,
                background: on ? accent : 'transparent',
                border: `1px solid ${on ? accent : OUTLINE}`,
              }}
            />
          );
        })}
      </div>
    );
  }
  if (shape === 'meter') {
    return (
      <div style={{display: 'flex', alignItems: 'flex-end', gap: u * 0.025, height: '100%', paddingTop: u * 0.06}}>
        {[0.5, 0.85, 0.4, 0.7, 0.95, 0.35, 0.6].map((h, i) => (
          <div key={i} style={{flex: 1, height: `${h * 100}%`, background: `${accent}D0`, borderRadius: u * 0.015}} />
        ))}
      </div>
    );
  }
  if (shape === 'summary') {
    return (
      <div style={{display: 'flex', flexDirection: 'column', gap: u * 0.045, justifyContent: 'space-evenly', height: '100%'}}>
        {[0, 1, 2, 3, 4].map((i) => (
          <div key={i} style={{display: 'flex', justifyContent: 'space-between', gap: u * 0.05}}>
            {line(`${50 - i * 5}%`, 0.035)}
            {line(`${16 + i * 3}%`, 0.035)}
          </div>
        ))}
      </div>
    );
  }
  if (shape.startsWith('chips')) {
    const c = Math.min(shapeCount(shape, 6), 8);
    return (
      <div style={{display: 'flex', flexWrap: 'wrap', gap: u * 0.028, alignContent: 'center', height: '100%'}}>
        {Array.from({length: c}, (_, i) => (
          <div
            key={i}
            style={{
              height: u * 0.1, borderRadius: 999, width: u * (0.16 + (i % 3) * 0.07),
              border: `1px solid ${i % 3 === 0 ? accent : OUTLINE}`,
              background: i % 3 === 0 ? `${accent}1E` : 'transparent',
            }}
          />
        ))}
      </div>
    );
  }
  if (shape.startsWith('toggle')) {
    const c = shapeCount(shape, 2);
    return (
      <div style={{display: 'flex', gap: u * 0.025, border: `1px solid ${OUTLINE}`, borderRadius: u * 0.045, padding: u * 0.02, marginTop: 'auto', marginBottom: 'auto'}}>
        {Array.from({length: c}, (_, i) => (
          <div key={i} style={{flex: 1, height: u * 0.11, borderRadius: u * 0.03, background: i === 0 ? accent : 'transparent'}} />
        ))}
      </div>
    );
  }
  if (shape.startsWith('list')) {
    const c = Math.min(shapeCount(shape, 4), 5);
    return (
      <div style={{display: 'flex', flexDirection: 'column', gap: u * 0.04, justifyContent: 'space-evenly', height: '100%'}}>
        {Array.from({length: c}, (_, i) => (
          <div key={i} style={{display: 'flex', gap: u * 0.045, alignItems: 'center'}}>
            <div
              style={{
                width: u * 0.1, height: u * 0.1, borderRadius: u * 0.028, flex: '0 0 auto',
                background: SURFACE_V, border: `1px solid ${i === 0 ? accent : OUTLINE}`,
              }}
            />
            {line(`${58 - i * 6}%`, 0.035)}
          </div>
        ))}
      </div>
    );
  }
  const c = Math.min(shapeCount(shape, 3), 4);
  return (
    <div style={{display: 'flex', flexDirection: 'column', gap: u * 0.042, height: '100%'}}>
      {Array.from({length: c}, (_, i) => (
        <div
          key={i}
          style={{
            flex: 1, minHeight: u * 0.12, borderRadius: u * 0.035,
            background: i === 0 ? `${accent}1E` : SURFACE_V,
            border: `1px solid ${i === 0 ? accent : OUTLINE}`,
          }}
        />
      ))}
    </div>
  );
};

const shapeCount = (s: Shape, d: number) => {
  const m = /-(\d+)$/.exec(s);
  return m ? Number(m[1]) : d;
};

export const PageCard: React.FC<{
  page: Page; w: number; accent: string; index: number; total: number; era: 'old' | 'new';
}> = ({page, w, accent, index, total, era}) => {
  const h = w * SHAPE_AR;
  const pad = w * 0.075;
  const old = era === 'old';
  const last = index === total - 1;
  const cta = old ? (last ? 'Let’s go' : 'Continue') : last ? 'Start training' : 'Continue';

  return (
    <div
      style={{
        width: w, height: h, borderRadius: w * 0.1, background: '#000',
        padding: Math.max(1.5, w * 0.012), boxSizing: 'border-box', flex: '0 0 auto',
        boxShadow: `0 0 0 1px ${OUTLINE}, 0 14px 34px rgba(0,0,0,0.55)`,
      }}
    >
      <div
        style={{
          width: '100%', height: '100%', borderRadius: w * 0.09, background: BG, overflow: 'hidden',
          padding: pad, boxSizing: 'border-box', display: 'flex', flexDirection: 'column', gap: w * 0.05,
        }}
      >
        {/* status bar — every real capture has one, so its absence is what reads as fake */}
        <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', height: w * 0.07}}>
          <div style={{width: w * 0.14, height: w * 0.035, background: MUTED, opacity: 0.5, borderRadius: 999}} />
          <div style={{display: 'flex', gap: w * 0.022}}>
            {[0, 1, 2].map((i) => (
              <div key={i} style={{width: w * 0.028, height: w * 0.028, borderRadius: 999, background: MUTED, opacity: 0.42}} />
            ))}
          </div>
        </div>

        {/* Tell one: 0.8.9's step bar was continuous and had to guess the path length before the
            plan-mode fork. 0.9 gives it one cell per step, so a short path visibly loses cells. */}
        {old ? (
          <div style={{height: w * 0.022, borderRadius: 999, background: `${OUTLINE}99`, overflow: 'hidden'}}>
            <div style={{width: `${((index + 1) / total) * 100}%`, height: '100%', background: accent}} />
          </div>
        ) : (
          <div style={{display: 'flex', gap: w * 0.018}}>
            {Array.from({length: total}, (_, i) => (
              <div key={i} style={{flex: 1, height: w * 0.022, borderRadius: 999, background: i <= index ? accent : `${OUTLINE}99`}} />
            ))}
          </div>
        )}

        {/* Tell two: the uppercase chapter eyebrow, which 0.9 deleted outright. */}
        <div style={{height: w * 0.06}}>
          {old && page.eyebrow ? (
            <div
              style={{
                fontFamily: MONO, fontSize: w * 0.052, letterSpacing: w * 0.008,
                color: accent, textTransform: 'uppercase', lineHeight: 1,
              }}
            >
              {page.eyebrow}
            </div>
          ) : null}
        </div>

        <div style={{fontFamily: SERIF, fontSize: w * 0.088, lineHeight: 1.18, color: ON_BG}}>
          {page.heading}
        </div>

        <div style={{display: 'flex', flexDirection: 'column', gap: w * 0.022}}>
          <div style={{width: '86%', height: w * 0.026, background: MUTED, opacity: 0.24, borderRadius: 999}} />
          <div style={{width: '54%', height: w * 0.026, background: MUTED, opacity: 0.24, borderRadius: 999}} />
        </div>

        {/* Tell three: the PlanLedger, new in 0.9 — the plan builds while you answer. */}
        {page.ledger ? (
          <div style={{display: 'flex', gap: w * 0.02, alignItems: 'flex-end', height: w * 0.085}}>
            {[0.45, 0.8, 0.3, 0.65, 0.5].map((f, i) => (
              <div key={i} style={{flex: 1, height: `${f * 100}%`, background: `${accent}CC`, borderRadius: w * 0.012}} />
            ))}
          </div>
        ) : null}

        <div style={{flex: 1, minHeight: 0, overflow: 'hidden'}}>
          <ShapeArt shape={page.shape} accent={accent} u={w} />
        </div>

        {/* The CTA is its own tell: 0.8.9 shipped a white pill, 0.9 an accent-filled slab. */}
        <div
          style={{
            height: w * 0.15, borderRadius: old ? 999 : w * 0.035,
            background: old ? ON_BG : accent,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            fontFamily: MONO, fontSize: w * 0.058, letterSpacing: w * 0.004,
            color: old ? BG : ON_BG, flex: '0 0 auto',
          }}
        >
          {cta}
        </div>
      </div>
    </div>
  );
};

/* ── the beat ────────────────────────────────────────────────────────────── */

type Phase = {layIn: number; bam: number};

export const OnboardingBeat: React.FC<{
  before: Page[]; after: Page[];
  eyebrow?: string; title?: string; line?: string;
  cols?: [number, number];
  phase?: Partial<Phase>;
  /** absolute first frame of the beat, so the pages can land on the music */
  gridStart?: number;
}> = ({before, after, eyebrow = 'Onboarding', title, line, cols, phase, gridStart = 0}) => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);

  // Absolute grid points, converted back to beat-local. The cards ARE the rhythm here, so they are
  // laid out from the grid and the springs follow them, not the other way round.
  const lay = run(gridStart + (phase?.layIn ?? QUARTER), before.length, SIXTEENTH).map((f) => f - gridStart);
  const bam = phase?.bam ?? Math.round(snap(gridStart + lay[lay.length - 1] + QUARTER * 2, 4) - gridStart);
  // The nine come back on sixteenths from the eighth after the hit, not on eighths from the next
  // quarter. On eighths the first page stood alone in an empty frame for a quarter of a second
  // before the second one arrived, which read as a mistake rather than a beginning; now the first
  // grows out of the impact point as the old pages clear and all nine are standing 1.3 s after the
  // hit. They arrive nearest-the-centre first, so the grid reads as blooming rather than typing.
  const bloomRun = run(gridStart + bam + EIGHTH, after.length, SIXTEENTH).map((f) => f - gridStart);
  const bloom = bloomRun[0];
  const bloomEnd = bloomRun[bloomRun.length - 1];

  // Two rows for both, but the block visibly narrows: fifteen small pages span ~1140px, nine
  // larger ones span ~850. The shrink is the point, so it has to be legible as a shape.
  const beforeCols = cols?.[0] ?? 8;
  const afterCols = cols?.[1] ?? 5;

  /*
   * The hit.
   *
   * The first cut faded the old grid out under a red glow while nudging it a few pixels toward the
   * centre. Rendered, that was a smear that stayed half-legible for eight frames, then six empty
   * frames, then one small page alone in the middle. Now the impact throws the fifteen pages
   * outward and off, fast and decelerating, gone in ten frames with the shock ring travelling with
   * them; the fade starts a frame after the motion so the throw reads before the fade does; and
   * the nine grow out of the point of impact as the last of them clears. At no frame are the two
   * grids both legible.
   */
  const blast = interpolate(frame, [bam, bam + 10], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.glide,
  });
  const fade = interpolate(frame, [bam + 1, bam + 8], [1, 0], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });
  const flash = interpolate(frame, [bam - 1, bam + 1, bam + 9], [0, 1, 0], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });
  const shock = interpolate(frame, [bam, bam + 18], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.snap,
  });

  const showBefore = frame < bam + 9;
  const showAfter = frame >= bloom - LEAD;

  const rise = useRise(4, 12);

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {/* Fifteen pages on sixteenths, the hit on a downbeat, nine pages back on sixteenths. Each
            page's spring starts LEAD frames before its cue, so the tap sounds as the page is standing
            rather than a sixth of a second before it appears. The taps and pops decay so a run of
            them reads as a flow rather than a drum roll. */}
        {lay.map((f, i) => <Cue key={i} at={f} sfx="tap" gain={0.55 * Math.pow(0.968, i)} />)}
        <Cue at={bam} sfx="impact" />
        {bloomRun.map((f, i) => <Cue key={i} at={f} sfx="pop" gain={0.6 * Math.pow(0.94, i)} />)}

        <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 30, width: 1560, marginTop: 52}}>
          <div style={{display: 'flex', alignItems: 'flex-end', justifyContent: 'space-between', width: '100%'}}>
            <div style={{display: 'flex', flexDirection: 'column', gap: 16}}>
              <Eyebrow delay={0}>{eyebrow}</Eyebrow>
              <Title delay={4} size={62}>{title ?? `${before.length} pages down to ${after.length}`}</Title>
            </div>
            <div style={{display: 'flex', alignItems: 'baseline', gap: 18, ...rise}}>
              <Ledger v={before.length} on={1 - blast} />
              <div style={{fontFamily: MONO, fontSize: 34, color: MUTED, opacity: 0.5}}>→</div>
              <Ledger v={after.length} on={blast} accent />
            </div>
          </div>

          <div style={{position: 'relative', width: '100%', minHeight: 668, display: 'flex', alignItems: 'center', justifyContent: 'center'}}>
            {showBefore ? (
              <Grid
                pages={before} cols={beforeCols} w={132} accent={ACCENT_OLD} era="old"
                enterAt={lay[0]} per={SIXTEENTH} exit={blast} fade={fade}
              />
            ) : null}
            {showAfter ? (
              <Grid
                pages={after} cols={afterCols} w={158} accent={ACCENT} era="new"
                enterAt={bloom} per={SIXTEENTH} bloom radial
              />
            ) : null}

            {/* the hit itself */}
            <div
              style={{
                position: 'absolute', inset: 0, pointerEvents: 'none',
                background: `radial-gradient(60% 60% at 50% 50%, ${ACCENT}55 0%, ${ACCENT}00 70%)`,
                opacity: flash,
              }}
            />
            {shock > 0 && shock < 1 ? (
              <div
                style={{
                  position: 'absolute', left: '50%', top: '50%', width: 40, height: 40, marginLeft: -20, marginTop: -20,
                  border: `2px solid ${ACCENT}`, borderRadius: 999, pointerEvents: 'none',
                  transform: `scale(${1 + shock * 34})`, opacity: (1 - shock) * 0.7,
                }}
              />
            ) : null}
          </div>

          {line ? (
            <div style={{textAlign: 'center', maxWidth: 1460}}>
              <Line delay={bloomEnd + 4} width={1460}>{line}</Line>
            </div>
          ) : null}
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

const Ledger: React.FC<{v: number; on: number; accent?: boolean}> = ({v, on, accent}) => (
  <div
    style={{
      fontFamily: SERIF, fontSize: 86, lineHeight: 1, fontVariantNumeric: 'tabular-nums',
      color: accent ? ACCENT : MUTED, opacity: 0.25 + on * 0.75,
      transform: `scale(${0.9 + on * 0.1})`,
    }}
  >
    {v}
  </div>
);

const Grid: React.FC<{
  pages: Page[]; cols: number; w: number; accent: string; era: 'old' | 'new';
  enterAt: number; per: number; exit?: number; fade?: number; bloom?: boolean;
  /** arrive nearest-the-centre first rather than in reading order */
  radial?: boolean;
}> = ({pages, cols, w, accent, era, enterAt, per, exit = 0, fade = 1, bloom = false, radial = false}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const rows = Math.ceil(pages.length / cols);
  const gap = bloom ? 15 : 12;
  const h = w * SHAPE_AR;
  const dist = (i: number) =>
    Math.hypot(((i % cols) - (cols - 1) / 2) * (w + gap), (Math.floor(i / cols) - (rows - 1) / 2) * (h + gap));
  const order = pages.map((_, i) => i);
  if (radial) order.sort((a, b) => dist(a) - dist(b) || a - b);
  const arrival = pages.map((_, i) => order.indexOf(i));
  return (
    <div
      style={{
        display: 'grid', gridTemplateColumns: `repeat(${cols}, ${w}px)`, gap,
        justifyContent: 'center', alignContent: 'center',
      }}
    >
      {pages.map((p, i) => {
        const s = spring({frame: frame - (enterAt + arrival[i] * per - LEAD), fps, config: {damping: 200, stiffness: bloom ? 190 : 210}});
        // This card's offset from the centre of the grid, which is where the hit lands.
        const col = i % cols;
        const row = Math.floor(i / cols);
        const ox = (col - (cols - 1) / 2) * (w + gap);
        const oy = (row - (rows - 1) / 2) * (h + gap);
        let tx: number, ty: number, sc: number, op: number;
        if (bloom) {
          // Grows out of the point of impact into its place.
          tx = -ox * (1 - s);
          ty = -oy * (1 - s);
          sc = 0.5 + 0.5 * s;
          op = s;
        } else {
          // Lands with a small rise; leaves thrown outward from the same point, further the further
          // from it the card sat, so the outer columns clear the frame.
          tx = ox * 0.9 * exit;
          ty = oy * 1.6 * exit + (1 - s) * 18;
          sc = (0.9 + s * 0.1) * (1 + exit * 0.2);
          op = s * fade;
        }
        return (
          <div key={i} style={{opacity: op, transform: `translate(${tx}px, ${ty}px) scale(${sc})`}}>
            <PageCard page={p} w={w} accent={accent} index={i} total={pages.length} era={era} />
          </div>
        );
      })}
    </div>
  );
};

/* ── the real flows ──────────────────────────────────────────────────────── */

/**
 * Both lists are the app's own page dispatch, in order, with `StepTitle` headings quoted verbatim.
 * The generated path is the fair one to count: it is the recommended pick and the only path where
 * both versions do comparable work. The counts are 15 → 9, which the 0.9 source states itself —
 * "Generated: mode → goal → experience → days → gym → gear → sore spots → week → extras (9)".
 * An earlier cut of this film claimed 17 → 10; neither tree produces those numbers.
 */
export const ONBOARDING_0_8_9: Page[] = [
  {name: 'Welcome',       heading: 'What do you go by?',              shape: 'text-field'},
  {name: 'Units',         heading: 'How do you measure?',             shape: 'toggle-2', eyebrow: 'About you'},
  {name: 'Body',          heading: 'About your body',                 shape: 'text-field', eyebrow: 'About you'},
  {name: 'Wearable',      heading: "What's on your wrist?",           shape: 'cards-3',  eyebrow: 'About you'},
  {name: 'App lock',      heading: 'Lock your data?',                 shape: 'toggle-2', eyebrow: 'About you'},
  {name: 'Plan mode',     heading: 'How do you want to train?',       shape: 'cards-3',  eyebrow: 'Your plan'},
  {name: 'Goal',          heading: "What's your main goal?",          shape: 'cards-4',  eyebrow: 'Goals'},
  {name: 'Experience',    heading: 'How long have you been training?', shape: 'cards-3', eyebrow: 'Goals'},
  {name: 'Days',          heading: 'How many days a week?',           shape: 'days',     eyebrow: 'Your gym'},
  {name: 'Gym',           heading: "What's in your gym?",             shape: 'list-4',   eyebrow: 'Your gym'},
  {name: 'Gear',          heading: 'Fine-tune your gear',             shape: 'list-5',   eyebrow: 'Your gym'},
  {name: 'Plate',         heading: 'Weight per plate',                shape: 'chips-6',  eyebrow: 'Your gym'},
  {name: 'Problem areas', heading: 'Any problem areas?',              shape: 'chips-8',  eyebrow: 'Fine-tuning'},
  {name: 'Cadence',       heading: 'Auto-refresh your plan?',         shape: 'cards-4',  eyebrow: 'Fine-tuning'},
  {name: 'Preview',       heading: "Here's your week",                shape: 'summary',  eyebrow: 'Your week'},
];

export const ONBOARDING_0_9: Page[] = [
  {name: 'Plan mode',  heading: 'How do you want to train?',        shape: 'cards-3'},
  {name: 'Goal',       heading: "What's your main goal?",           shape: 'cards-4'},
  {name: 'Experience', heading: 'How long have you been training?', shape: 'cards-3'},
  {name: 'Days',       heading: 'How many days a week?',            shape: 'days',   ledger: true},
  {name: 'Gym',        heading: "What's in your gym?",              shape: 'list-4', ledger: true},
  {name: 'Gear',       heading: 'Fine-tune your gear',              shape: 'list-5', ledger: true},
  {name: 'Sore spots', heading: 'Any sore or injured spots?',       shape: 'list-4', ledger: true},
  {name: 'Week',       heading: "Here's your week",                 shape: 'meter'},
  {name: 'Extras',     heading: 'Anything else?',                   shape: 'list-5'},
];
