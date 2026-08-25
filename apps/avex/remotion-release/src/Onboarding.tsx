import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {EASE} from './Camera';
import {Plate} from './Layout';
import {Cue, CueRun} from './Sound';
import {Eyebrow, Line, Title, useEdgeFade, useRise} from './Type';
import {ACCENT, ACCENT_OLD, MONO, MUTED, ON_BG, OUTLINE, SERIF, SURFACE, SURFACE_V} from './theme';

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

const n = (s: Shape, d = 3) => {
  const m = /-(\d+)$/.exec(s);
  return m ? Number(m[1]) : d;
};

/* ── one page, in miniature ──────────────────────────────────────────────── */

const Bar: React.FC<{w: string | number; h: number; c?: string; r?: number}> = ({w, h, c = OUTLINE, r = 3}) => (
  <div style={{width: w, height: h, background: c, borderRadius: r, flex: '0 0 auto'}} />
);

const ShapeArt: React.FC<{shape: Shape; accent: string}> = ({shape, accent}) => {
  const box = (h: number, fill?: string) => (
    <div style={{height: h, borderRadius: 5, background: fill ?? SURFACE_V, border: `1px solid ${OUTLINE}`}} />
  );
  if (shape === 'text-field') {
    return (
      <div style={{display: 'flex', flexDirection: 'column', gap: 7}}>
        <div style={{height: 22, borderRadius: 5, border: `1px solid ${OUTLINE}`, background: SURFACE_V, display: 'flex', alignItems: 'center', paddingLeft: 7}}>
          <Bar w={2} h={11} c={accent} r={1} />
        </div>
      </div>
    );
  }
  if (shape === 'slider') {
    return (
      <div style={{display: 'flex', flexDirection: 'column', gap: 10, paddingTop: 6}}>
        <div style={{height: 4, borderRadius: 999, background: OUTLINE, position: 'relative'}}>
          <div style={{position: 'absolute', inset: 0, width: '58%', background: accent, borderRadius: 999}} />
          <div style={{position: 'absolute', left: '58%', top: -5, width: 14, height: 14, marginLeft: -7, borderRadius: 999, background: accent}} />
        </div>
        <Bar w="34%" h={7} />
      </div>
    );
  }
  if (shape === 'days') {
    return (
      <div style={{display: 'flex', gap: 4, justifyContent: 'space-between'}}>
        {Array.from({length: 7}, (_, i) => (
          <div
            key={i}
            style={{
              flex: 1, height: 24, borderRadius: 5,
              background: [1, 3, 5].includes(i) ? accent : 'transparent',
              border: `1px solid ${[1, 3, 5].includes(i) ? accent : OUTLINE}`,
            }}
          />
        ))}
      </div>
    );
  }
  if (shape === 'meter') {
    return (
      <div style={{display: 'flex', flexDirection: 'column', gap: 9, paddingTop: 4}}>
        {[0.8, 0.55, 0.35].map((f, i) => (
          <div key={i} style={{height: 7, borderRadius: 999, background: `${OUTLINE}80`, overflow: 'hidden'}}>
            <div style={{width: `${f * 100}%`, height: '100%', background: accent, borderRadius: 999}} />
          </div>
        ))}
      </div>
    );
  }
  if (shape === 'summary') {
    return (
      <div style={{display: 'flex', flexDirection: 'column', gap: 6}}>
        {[0, 1, 2, 3].map((i) => (
          <div key={i} style={{display: 'flex', justifyContent: 'space-between', gap: 8}}>
            <Bar w={`${52 - i * 6}%`} h={6} />
            <Bar w={`${18 + i * 3}%`} h={6} c={i === 0 ? accent : OUTLINE} />
          </div>
        ))}
      </div>
    );
  }
  if (shape.startsWith('chips')) {
    const c = n(shape, 6);
    return (
      <div style={{display: 'flex', flexWrap: 'wrap', gap: 5}}>
        {Array.from({length: Math.min(c, 9) }, (_, i) => (
          <div
            key={i}
            style={{
              height: 17, borderRadius: 999, paddingLeft: 9, paddingRight: 9,
              border: `1px solid ${i % 3 === 0 ? accent : OUTLINE}`,
              background: i % 3 === 0 ? `${accent}22` : 'transparent',
              width: 26 + (i % 3) * 13,
            }}
          />
        ))}
      </div>
    );
  }
  if (shape.startsWith('toggle')) {
    const c = n(shape, 2);
    return (
      <div style={{display: 'flex', gap: 5, border: `1px solid ${OUTLINE}`, borderRadius: 6, padding: 3}}>
        {Array.from({length: c}, (_, i) => (
          <div key={i} style={{flex: 1, height: 18, borderRadius: 4, background: i === 0 ? accent : 'transparent'}} />
        ))}
      </div>
    );
  }
  if (shape.startsWith('list')) {
    const c = Math.min(n(shape, 4), 5);
    return (
      <div style={{display: 'flex', flexDirection: 'column', gap: 6}}>
        {Array.from({length: c}, (_, i) => (
          <div key={i} style={{display: 'flex', gap: 7, alignItems: 'center'}}>
            <Bar w={14} h={14} c={SURFACE_V} r={4} />
            <Bar w={`${62 - i * 7}%`} h={6} />
          </div>
        ))}
      </div>
    );
  }
  // cards-N
  const c = Math.min(n(shape, 3), 4);
  return (
    <div style={{display: 'flex', flexDirection: 'column', gap: 6}}>
      {Array.from({length: c}, (_, i) => (
        <div key={i} style={{position: 'relative'}}>
          {box(i === 0 ? 26 : 22, i === 0 ? `${accent}1A` : undefined)}
          {i === 0 ? (
            <div style={{position: 'absolute', inset: 0, borderRadius: 5, border: `1px solid ${accent}`}} />
          ) : null}
        </div>
      ))}
    </div>
  );
};

export const PageCard: React.FC<{
  page: Page; w: number; h: number; accent: string; index: number; total: number; era: 'old' | 'new';
}> = ({page, w, h, accent, index, total, era}) => (
  <div
    style={{
      width: w, height: h, overflow: 'hidden',
      background: SURFACE, border: `1px solid ${OUTLINE}`, borderRadius: 12,
      padding: 13, display: 'flex', flexDirection: 'column', gap: 8, boxSizing: 'border-box',
      boxShadow: '0 10px 26px rgba(0,0,0,0.45)',
    }}
  >
    {/* Tell one: 0.8.9's continuous bar had to guess the path length before the plan-mode fork,
        so 0.9 replaced it with a segment per page. At card size it is the difference you see first. */}
    {era === 'old' ? (
      <div style={{height: 3, borderRadius: 999, background: `${OUTLINE}99`, overflow: 'hidden'}}>
        <div style={{width: `${((index + 1) / total) * 100}%`, height: '100%', background: accent}} />
      </div>
    ) : (
      <div style={{display: 'flex', gap: 3}}>
        {Array.from({length: total}, (_, i) => (
          <div key={i} style={{flex: 1, height: 3, borderRadius: 999, background: i <= index ? accent : `${OUTLINE}99`}} />
        ))}
      </div>
    )}

    {/* Tell two: the uppercase chapter eyebrow, which 0.9 deleted outright. */}
    {era === 'old' ? (
      <div style={{fontFamily: MONO, fontSize: 8.5, letterSpacing: 1.5, color: accent, textTransform: 'uppercase', height: 11}}>
        {page.eyebrow ?? ''}
      </div>
    ) : (
      <div style={{height: 11}} />
    )}

    <div style={{fontFamily: SERIF, fontSize: 13.5, lineHeight: 1.2, color: ON_BG, minHeight: 33}}>
      {page.heading}
    </div>

    {/* Tell three: the PlanLedger, new in 0.9 — the page answers back as you fill it in. */}
    {page.ledger ? (
      <div style={{display: 'flex', gap: 3, alignItems: 'flex-end', height: 13}}>
        {[0.5, 0.8, 0.35, 0.65, 0.45].map((h, i) => (
          <div key={i} style={{flex: 1, height: `${h * 100}%`, background: `${accent}CC`, borderRadius: 1.5}} />
        ))}
      </div>
    ) : null}

    <div style={{marginTop: 'auto'}}>
      <ShapeArt shape={page.shape} accent={accent} />
    </div>
  </div>
);

/* ── the beat ────────────────────────────────────────────────────────────── */

type Phase = {layIn: number; per: number; hold: number; bam: number; bloom: number; bloomPer: number};

export const OnboardingBeat: React.FC<{
  before: Page[]; after: Page[];
  eyebrow?: string; title?: string; line?: string;
  cols?: [number, number];
  phase?: Partial<Phase>;
}> = ({before, after, eyebrow = 'Onboarding', title, line, cols, phase}) => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);

  const P: Phase = {
    layIn: 6, per: 2.2, hold: 10, bam: 0, bloom: 0, bloomPer: 3.2,
    ...phase,
  };
  const layEnd = P.layIn + before.length * P.per + 14;   // last card still needs its spring
  const bam = P.bam || layEnd + P.hold;
  const bloom = P.bloom || bam + 7;

  const beforeCols = cols?.[0] ?? Math.ceil(before.length / 3);
  const afterCols = cols?.[1] ?? Math.ceil(after.length / 2);

  // The hit: a flash, a shockwave ring, and the old grid collapsing toward the centre.
  const hit = spring({frame: frame - bam, fps, config: {damping: 200, stiffness: 220}});
  const flash = interpolate(frame, [bam - 1, bam + 2, bam + 12], [0, 1, 0], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });
  const shock = interpolate(frame, [bam, bam + 22], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.snap,
  });

  const showBefore = frame < bam + 10;
  const showAfter = frame >= bloom - 2;

  const rise = useRise(4, 12);

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {/* every page landing is a real tap; they decay so seventeen of them do not become a drum roll */}
        <CueRun from={P.layIn + 6} every={P.per} count={before.length} sfx="tap" gain={0.5} decay={0.965} />
        <Cue at={bam} sfx="impact" />
        <CueRun from={bloom + 4} every={P.bloomPer} count={after.length} sfx="tick" gain={0.45} decay={0.97} />

        <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 34, width: 1560}}>
          <div style={{display: 'flex', alignItems: 'flex-end', justifyContent: 'space-between', width: '100%'}}>
            <div style={{display: 'flex', flexDirection: 'column', gap: 16}}>
              <Eyebrow delay={0}>{eyebrow}</Eyebrow>
              <Title delay={4} size={62}>{title ?? `${before.length} pages down to ${after.length}`}</Title>
            </div>
            <div style={{display: 'flex', alignItems: 'baseline', gap: 18, ...rise}}>
              <Ledger v={before.length} on={1 - hit} />
              <div style={{fontFamily: MONO, fontSize: 34, color: MUTED, opacity: 0.5}}>→</div>
              <Ledger v={after.length} on={hit} accent />
            </div>
          </div>

          <div style={{position: 'relative', width: '100%', minHeight: 468, display: 'flex', alignItems: 'center', justifyContent: 'center'}}>
            {showBefore ? (
              <Grid
                pages={before} cols={beforeCols} w={214} h={152} accent={ACCENT_OLD} era="old"
                enterAt={P.layIn} per={P.per} exitAt={bam} exit={hit}
              />
            ) : null}
            {showAfter ? (
              <Grid
                pages={after} cols={afterCols} w={262} h={186} accent={ACCENT} era="new"
                enterAt={bloom} per={P.bloomPer} bloom
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
            <div style={{textAlign: 'center', maxWidth: 1180}}>
              <Line delay={bloom + 6} width={1180}>{line}</Line>
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
  pages: Page[]; cols: number; w: number; h: number; accent: string; era: 'old' | 'new';
  enterAt: number; per: number; exitAt?: number; exit?: number; bloom?: boolean;
}> = ({pages, cols, w, h, accent, era, enterAt, per, exit = 0, bloom = false}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  return (
    <div
      style={{
        display: 'grid', gridTemplateColumns: `repeat(${cols}, ${w}px)`, gap: 18,
        justifyContent: 'center', alignContent: 'center',
      }}
    >
      {pages.map((p, i) => {
        const s = spring({frame: frame - (enterAt + i * per), fps, config: {damping: 200, stiffness: bloom ? 170 : 210}});
        // Cards leave by falling toward the centre of the grid, not straight down: the flow is being
        // compressed, and the motion should say compression.
        const col = i % cols;
        const row = Math.floor(i / cols);
        const dx = (cols / 2 - 0.5 - col) * 46 * exit;
        const dy = (Math.ceil(pages.length / cols) / 2 - 0.5 - row) * 40 * exit;
        return (
          <div
            key={i}
            style={{
              opacity: s * (1 - exit),
              transform: `translate(${dx}px, ${dy + (1 - s) * (bloom ? 26 : 18)}px) scale(${(0.9 + s * 0.1) * (1 - exit * 0.16)})`,
            }}
          >
            <PageCard page={p} w={w} h={h} accent={accent} index={i} total={pages.length} era={era} />
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
