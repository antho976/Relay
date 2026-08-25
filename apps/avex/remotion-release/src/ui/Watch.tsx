import React from 'react';

/**
 * The Wear OS app, transcribed from forge-android/wear — WearTheme.kt for the palette and type,
 * SessionScreen.kt / TimerView.kt for the layouts. Doctrine §16: pure black ground, ONE serif
 * figure per screen, mono uppercase micro-labels, the accent on the 1.0 / 0.6 / 0.15 ladder.
 *
 * Authored in dp against a 227dp round display (a 454px Galaxy Watch at 2x).
 */
export const W = {
  onBg: '#F2EFEA',
  muted: '#BFB6AA',
  outline: '#38302A',
  accent: '#E23D3D',
  accentDim: 'rgba(226,61,61,0.6)',
  accentWash: 'rgba(226,61,61,0.15)',
  prGold: '#E3B341',
  ground: '#000000',
};

const SERIF = 'Georgia, "Times New Roman", ui-serif, serif';
const SANS = 'Inter, "Helvetica Neue", Arial, ui-sans-serif, sans-serif';
const MONO = '"JetBrains Mono", "DejaVu Sans Mono", ui-monospace, monospace';

const WT = {
  figure:      (k: number) => ({fontFamily: SERIF, fontSize: 40 * k, lineHeight: 1.02}),
  figureSmall: (k: number) => ({fontFamily: SERIF, fontSize: 26 * k, lineHeight: 1.05}),
  body:        (k: number) => ({fontFamily: SANS,  fontSize: 14 * k}),
  control:     (k: number) => ({fontFamily: SANS,  fontSize: 16 * k, fontWeight: 500}),
  label:       (k: number) => ({fontFamily: MONO,  fontSize: 10 * k, letterSpacing: 0.6 * k}),
  labelSmall:  (k: number) => ({fontFamily: MONO,  fontSize: 8 * k,  letterSpacing: 0.5 * k}),
};

const DP = 227; // the round display, in dp

export const WatchBody: React.FC<{size: number; children: React.ReactNode; glow?: number}> = ({
  size, children, glow = 0,
}) => {
  const k = size / DP;
  return (
    <div style={{position: 'relative', width: size, height: size, flex: '0 0 auto'}}>
      {/* case */}
      <div
        style={{
          position: 'absolute', inset: -10 * k, borderRadius: '50%',
          background: 'linear-gradient(150deg,#2a2724,#141210 55%,#0b0a09)',
          boxShadow: `0 26px 60px rgba(0,0,0,.65), inset 0 0 0 ${1.5 * k}px #3a352f`,
        }}
      />
      {/* crown */}
      <div
        style={{
          position: 'absolute', right: -16 * k, top: '41%', width: 8 * k, height: 26 * k,
          borderRadius: 3 * k, background: 'linear-gradient(90deg,#38332e,#1d1a17)',
        }}
      />
      {/* display */}
      <div
        style={{
          position: 'absolute', inset: 0, borderRadius: '50%', background: W.ground,
          overflow: 'hidden', display: 'flex', alignItems: 'center', justifyContent: 'center',
          boxShadow: glow ? `inset 0 0 ${40 * k}px rgba(226,61,61,${0.16 * glow})` : undefined,
        }}
      >
        <div style={{width: '100%', padding: `0 ${20 * k}px`, boxSizing: 'border-box'}}>{children}</div>
      </div>
    </div>
  );
};

const Center: React.FC<{children: React.ReactNode; gap?: number; k: number}> = ({children, gap = 2, k}) => (
  <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: gap * k}}>{children}</div>
);

const Capsule: React.FC<{label: string; k: number; accent?: boolean}> = ({label, k, accent = true}) => (
  <div
    style={{
      ...WT.control(k), color: accent ? '#110F0C' : W.onBg,
      background: accent ? W.accent : 'transparent',
      border: accent ? 'none' : `${1.2 * k}px solid ${W.outline}`,
      borderRadius: 999, padding: `${7 * k}px ${22 * k}px`,
    }}
  >
    {label}
  </div>
);

const Stepper: React.FC<{s: string; k: number}> = ({s, k}) => (
  <div
    style={{
      width: 30 * k, height: 30 * k, borderRadius: '50%', border: `${1.2 * k}px solid ${W.outline}`,
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      ...WT.control(k), color: W.muted,
    }}
  >
    {s}
  </div>
);

/** SetView — the screen you actually lift from. */
export const WatchSet: React.FC<{
  size: number; day?: string; min?: number; exercise?: string;
  weight: string; reps: string; unit?: string; set?: number; total?: number; bpm?: number;
}> = ({
  size, day = 'UPPER A', min = 12, exercise = 'INCLINE BARBELL BENCH',
  weight, reps, unit = 'LB', set = 2, total = 4, bpm = 132,
}) => {
  const k = size / DP;
  return (
    <WatchBody size={size}>
      <Center k={k} gap={2}>
        <div style={{...WT.labelSmall(k), color: W.muted}}>{`${day} · ${min} MIN`}</div>
        <div style={{...WT.label(k), color: W.onBg, textAlign: 'center', marginTop: 2 * k}}>{exercise}</div>
        <div style={{display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%', marginTop: 8 * k}}>
          <Stepper s="−" k={k} />
          <div style={{...WT.figure(k), color: W.onBg, fontVariantNumeric: 'tabular-nums'}}>{weight}</div>
          <Stepper s="+" k={k} />
        </div>
        <div style={{display: 'flex', alignItems: 'center', gap: 8 * k, marginTop: 2 * k}}>
          <span style={{...WT.labelSmall(k), color: W.muted}}>{unit}</span>
          <span style={{...WT.label(k), color: W.onBg}}>{`× ${reps}`}</span>
        </div>
        <div style={{...WT.labelSmall(k), color: W.muted, marginTop: 6 * k}}>
          {`SET ${set} OF ${total} · ${bpm} BPM`}
        </div>
        <div style={{marginTop: 8 * k}}><Capsule label="Log set" k={k} /></div>
        <div style={{display: 'flex', gap: 22 * k, marginTop: 7 * k}}>
          <span style={{...WT.label(k), color: W.accent}}>undo</span>
          <span style={{...WT.label(k), color: W.accent}}>rate →</span>
        </div>
      </Center>
    </WatchBody>
  );
};

/** TimerView — the rest countdown, with the ring draining. */
export const WatchRest: React.FC<{size: number; remaining: number; total: number}> = ({
  size, remaining, total,
}) => {
  const k = size / DP;
  const mm = Math.floor(Math.max(0, remaining) / 60);
  const ss = Math.max(0, Math.floor(remaining % 60));
  const p = Math.max(0, Math.min(1, remaining / total));
  const r = (DP / 2 - 7) * k;
  const c = 2 * Math.PI * r;
  return (
    <WatchBody size={size}>
      <svg width={size} height={size} style={{position: 'absolute', inset: 0, transform: 'rotate(-90deg)'}}>
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={W.outline} strokeWidth={4 * k} />
        <circle
          cx={size / 2} cy={size / 2} r={r} fill="none" stroke={W.accent} strokeWidth={4 * k}
          strokeLinecap="round" strokeDasharray={c} strokeDashoffset={c * (1 - p)}
        />
      </svg>
      <Center k={k} gap={4}>
        <div style={{...WT.labelSmall(k), color: W.muted}}>REST</div>
        <div style={{...WT.figure(k), color: W.onBg, fontVariantNumeric: 'tabular-nums'}}>
          {`${mm}:${String(ss).padStart(2, '0')}`}
        </div>
        <div style={{marginTop: 8 * k}}><Capsule label="Skip" k={k} accent={false} /></div>
      </Center>
    </WatchBody>
  );
};

/** RpeScreen — how hard was that, and what's left in the tank. */
export const WatchRpe: React.FC<{size: number; rpe: number}> = ({size, rpe}) => {
  const k = size / DP;
  const rir = (10 - rpe).toFixed(1).replace(/\.0$/, '');
  return (
    <WatchBody size={size}>
      <Center k={k} gap={4}>
        <div style={{...WT.labelSmall(k), color: W.muted}}>RPE · HOW HARD</div>
        <div style={{display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%'}}>
          <Stepper s="−" k={k} />
          <div style={{...WT.figure(k), color: W.onBg, fontVariantNumeric: 'tabular-nums'}}>
            {rpe.toFixed(1).replace(/\.0$/, '')}
          </div>
          <Stepper s="+" k={k} />
        </div>
        <div style={{...WT.labelSmall(k), color: W.muted}}>{`${rir} RIR`}</div>
        <div style={{marginTop: 10 * k}}><Capsule label="Save" k={k} /></div>
        <div style={{...WT.label(k), color: W.accent, marginTop: 7 * k}}>close</div>
      </Center>
    </WatchBody>
  );
};

/**
 * The PR beat. Doctrine reserves gold for one event and the watch keeps that promise: a set that
 * beats the record answers in `prGold`, never the accent, and the app's other confirmations stay
 * red so this one reads as different in a way a louder red never could.
 */
export const WatchPr: React.FC<{size: number; lift?: string; value: string; on?: number}> = ({
  size, lift = 'INCLINE BARBELL BENCH', value, on = 1,
}) => {
  const k = size / DP;
  return (
    <div style={{position: 'relative'}}>
      <WatchBody size={size}>
        <Center k={k} gap={4}>
          <div style={{...WT.labelSmall(k), color: W.prGold, letterSpacing: 2 * k}}>NEW PR</div>
          <div style={{...WT.figure(k), color: W.prGold, fontVariantNumeric: 'tabular-nums'}}>{value}</div>
          <div style={{...WT.label(k), color: W.muted, textAlign: 'center'}}>{lift}</div>
        </Center>
      </WatchBody>
      <div
        style={{
          position: 'absolute', inset: -10 * k, borderRadius: '50%', pointerEvents: 'none',
          boxShadow: `0 0 ${70 * k * on}px ${10 * k * on}px rgba(227,179,65,${0.42 * on})`,
          background: `radial-gradient(circle, rgba(227,179,65,${0.2 * on}) 0%, rgba(227,179,65,0) 68%)`,
        }}
      />
    </div>
  );
};
