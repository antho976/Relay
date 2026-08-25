import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {HomeScreen} from './ui/HomeScreen';
import {GoalRow, ScaleCtx} from './ui/kit';
import {Icon} from './ui/icons';
import {C} from './ui/tokens';
import {WatchRest, WatchRpe, WatchSet} from './ui/Watch';
import {Body, Eyebrow, Plate, Title, useEdgeFade} from './Type';

const MONO = '"JetBrains Mono", ui-monospace, monospace';

/** Home, morphing 0.8.9 → 0.9 in place. No cut, so every change lands on one clock. */
export const HomeMorph: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);
  const t = spring({frame: frame - 55, fps, config: {damping: 200, stiffness: 26}});
  const fill = interpolate(frame, [12, 62], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', width: '100%', height: '100%', alignItems: 'center'}}>
          <div style={{flex: '0 0 700px', paddingLeft: 110, display: 'flex', flexDirection: 'column', gap: 22}}>
            <Eyebrow delay={0}>Home</Eyebrow>
            <Title delay={4} size={72}>One screen,{'\n'}two eras</Title>
            <Body delay={12}>
              The wordmark becomes the bell. The pill becomes the accent, and gives up half its width
              to Plan. The meters bleed navy to red, and the fifth tab stops being you.
            </Body>
            <div style={{display: 'flex', gap: 18, marginTop: 10, alignItems: 'center'}}>
              <Era label="0.8.9" on={1 - t} />
              <div style={{width: 200, height: 3, borderRadius: 999, background: C.outline, position: 'relative'}}>
                <div style={{position: 'absolute', inset: 0, width: `${t * 100}%`, background: C.accent, borderRadius: 999}} />
              </div>
              <Era label="0.9" on={t} accent />
            </div>
          </div>
          <div style={{flex: 1, display: 'flex', justifyContent: 'center'}}>
            <div style={{transform: `translateY(${interpolate(t, [0, 1], [0, -8])}px)`, filter: 'drop-shadow(0 40px 90px rgba(0,0,0,.6))'}}>
              <HomeScreen width={430} t={t} goalFill={fill} />
            </div>
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

const Era: React.FC<{label: string; on: number; accent?: boolean}> = ({label, on, accent}) => (
  <div
    style={{
      fontFamily: MONO, fontSize: 20, letterSpacing: 3, color: accent ? C.accent : C.muted,
      opacity: 0.3 + on * 0.7, border: `1.5px solid ${accent ? C.accent : C.outline}`,
      borderRadius: 999, padding: '8px 20px',
    }}
  />
);

/**
 * A single goal row, lifted off the screen and blown up. This is the beat a screen recording cannot
 * give you: the component alone, at a size where the meter and the implement chip are the subject.
 */
export const GoalCloseUp: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);
  const t = spring({frame: frame - 50, fps, config: {damping: 200, stiffness: 30}});
  const fill = interpolate(frame, [14, 70], [0, 0.94], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const accent = t > 0.5 ? C.accent : C.accentOld;

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', flexDirection: 'column', gap: 46, width: 1400}}>
          <div style={{display: 'flex', flexDirection: 'column', gap: 18}}>
            <Eyebrow delay={0}>Goals</Eyebrow>
            <Title delay={4} size={76}>A meter that means something</Title>
          </div>
          {/* 3.4x the on-device size */}
          <ScaleCtx.Provider value={3.4}>
            <div style={{background: C.surface, borderRadius: 26, padding: '46px 52px', border: `1px solid ${C.outline}`}}>
              <GoalRow
                name="Incline Barbell Bench" cur={150} target={160}
                fill={fill} accent={accent} chip={t > 0.4} glyph="barbell"
              />
            </div>
          </ScaleCtx.Provider>
          <div style={{display: 'flex', gap: 40, opacity: interpolate(t, [0.3, 1], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'})}}>
            <Note n="01" v="The bar takes the accent — it used to fill neutral" />
            <Note n="02" v="The implement rides alongside the name" />
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

const Note: React.FC<{n: string; v: string}> = ({n, v}) => (
  <div style={{display: 'flex', gap: 14, alignItems: 'baseline'}}>
    <span style={{fontFamily: MONO, fontSize: 18, color: C.accent}}>{n}</span>
    <span style={{fontFamily: 'Georgia, serif', fontSize: 28, color: C.muted}}>{v}</span>
  </div>
);

/** The fifth tab changing hands, isolated from everything else. */
export const TabSwap: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);
  const t = spring({frame: frame - 46, fps, config: {damping: 200, stiffness: 30}});
  const tabs = [
    {label: 'Cardio', icon: 'cardio' as const},
    {label: 'Stats', icon: 'stats' as const},
    {label: 'Home', icon: 'home' as const},
    {label: 'Coach', icon: 'coach' as const},
  ];
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', flexDirection: 'column', gap: 60, alignItems: 'center'}}>
          <div style={{display: 'flex', flexDirection: 'column', gap: 16, alignItems: 'center'}}>
            <Eyebrow delay={0}>The fifth tab</Eyebrow>
            <Title delay={4} size={78}>Academy takes the slot</Title>
          </div>
          <div
            style={{
              display: 'flex', gap: 78, alignItems: 'flex-end',
              background: C.surface, border: `1px solid ${C.outline}`, borderRadius: 24, padding: '38px 62px',
            }}
          >
            {tabs.map((x, i) => (
              <Tab key={i} label={x.label} icon={x.icon} on={i === 2} />
            ))}
            <div style={{position: 'relative', width: 128, height: 92}}>
              <div style={{position: 'absolute', inset: 0, opacity: 1 - t, transform: `translateY(${t * -18}px)`}}>
                <Tab label="Profile" icon="profile" on={false} />
              </div>
              <div style={{position: 'absolute', inset: 0, opacity: t, transform: `translateY(${(1 - t) * 18}px)`}}>
                <Tab label="Academy" icon="academy" on={false} badge />
              </div>
            </div>
          </div>
          <div style={{fontFamily: 'Georgia, serif', fontSize: 30, fontStyle: 'italic', color: C.muted, maxWidth: 900, textAlign: 'center'}}>
            Profile moves up into Home&apos;s top bar. Academy was a link buried inside Coach — and it is
            half the coach, not a footnote to it.
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

const Tab: React.FC<{label: string; icon: any; on: boolean; badge?: boolean}> = ({label, icon, on, badge}) => (
  <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 12, width: 128, position: 'relative'}}>
    <Icon name={icon} size={44} color={on ? C.accent : C.muted} />
    <div style={{fontFamily: MONO, fontSize: 19, letterSpacing: 1, color: on ? C.accent : C.muted}}>{label}</div>
    {badge ? (
      <div style={{position: 'absolute', top: -8, right: 22, background: C.accent, borderRadius: 999, padding: '2px 8px', fontFamily: MONO, fontSize: 15, color: C.onBg}}>
        9+
      </div>
    ) : null}
  </div>
);

/** Lifetime volume, counted rather than shown. */
export const VolumeCount: React.FC = () => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);
  const p = interpolate(frame, [16, 96], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const eased = 1 - Math.pow(1 - p, 3);
  const v = Math.round(eased * 1486900);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 26}}>
          <Eyebrow delay={0}>Six months, seeded</Eyebrow>
          <div style={{fontFamily: 'Georgia, serif', fontSize: 190, color: C.onBg, lineHeight: 1, fontVariantNumeric: 'tabular-nums'}}>
            {v.toLocaleString('en-US')}
          </div>
          <div style={{fontFamily: MONO, fontSize: 26, letterSpacing: 5, color: C.accent}}>POUNDS MOVED</div>
          <div style={{fontFamily: 'Georgia, serif', fontStyle: 'italic', fontSize: 29, color: C.muted, marginTop: 12}}>
            92 sessions · 1,859 sets · 58 cardio days
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

/* ── Wear OS ─────────────────────────────────────────────────────────────── */

/**
 * The watch, walking its real flow: set → log → rest → rate. One screen at a time, because that is
 * the watch's own doctrine (§16), not a layout convenience.
 */
export const WatchBeat: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);

  // phase boundaries, in frames
  const P = {set: 0, log: 78, rest: 96, rpe: 216};
  const stepIn = spring({frame: frame - 24, fps, config: {damping: 200, stiffness: 60}});
  const weight = 150 + Math.round(stepIn * 5); // the ± stepper ticking 150 → 155

  const phase = frame >= P.rpe ? 'rpe' : frame >= P.rest ? 'rest' : 'set';
  const restT = interpolate(frame, [P.rest, P.rpe - 10], [178, 92], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});

  const enter = (at: number) =>
    spring({frame: frame - at, fps, config: {damping: 200, stiffness: 90}});
  const eSet = 1 - enter(P.rest);
  const eRest = enter(P.rest) * (1 - enter(P.rpe));
  const eRpe = enter(P.rpe);

  const label = phase === 'rpe' ? 'Rate it, or don’t' : phase === 'rest' ? 'The rest is kept for you' : 'Adjust and log, on the wrist';

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', width: '100%', height: '100%', alignItems: 'center'}}>
          <div style={{flex: '0 0 720px', paddingLeft: 110, display: 'flex', flexDirection: 'column', gap: 22}}>
            <Eyebrow delay={0}>New in 0.9 · Wear OS</Eyebrow>
            <Title delay={4} size={74}>Leave the phone{'\n'}in your bag</Title>
            <Body delay={12}>
              A companion watch app that mirrors the live session: adjust the load, log the set, and
              the rest timer runs on your wrist. Heart rate rides along, and it writes back through
              Health Connect.
            </Body>
            <div style={{marginTop: 16, display: 'flex', gap: 14, alignItems: 'center', minHeight: 40}}>
              <span style={{fontFamily: MONO, fontSize: 18, color: C.accent, letterSpacing: 2}}>
                {phase === 'set' ? '01' : phase === 'rest' ? '02' : '03'}
              </span>
              <span style={{fontFamily: 'Georgia, serif', fontSize: 32, color: C.onBg}}>{label}</span>
            </div>
          </div>
          <div style={{flex: 1, display: 'flex', justifyContent: 'center', position: 'relative', height: 620}}>
            <Layer on={eSet}>
              <WatchSet size={430} weight={String(weight)} reps="9" />
            </Layer>
            <Layer on={eRest}>
              <WatchRest size={430} remaining={restT} total={210} />
            </Layer>
            <Layer on={eRpe}>
              <WatchRpe size={430} rpe={8.5} />
            </Layer>
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

const Layer: React.FC<{on: number; children: React.ReactNode}> = ({on, children}) => (
  <div
    style={{
      position: 'absolute', top: '50%', left: '50%',
      transform: `translate(-50%,-50%) scale(${0.94 + on * 0.06})`,
      opacity: on, pointerEvents: 'none',
    }}
  >
    {children}
  </div>
);
