import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {HomeScreen} from './ui/HomeScreen';
import {Icon} from './ui/icons';
import {C} from './ui/tokens';
import {WatchRest, WatchRpe, WatchSet} from './ui/Watch';
import {Body, Eyebrow, Title, useEdgeFade} from './Type';
import {Plate} from './Layout';
import {Cue, LEAD} from './Sound';
import {Device} from './Device';
import {SHOT_AR, snap} from './theme';

const MONO = '"JetBrains Mono", ui-monospace, monospace';

/**
 * Home, morphing 0.8.9 → 0.9 in place. No cut, so every change lands on one clock.
 *
 * `turnAt` is the frame the era turns over on. The release cut puts it on bar 5, the bar the bed
 * enters: the accent going red and the music arriving are one event. It used to sit a bar earlier,
 * in the silence before the bed, with a glass-xylophone run under the goal bars filling — the
 * loudest thing in the first ten seconds of the film and the first note anyone gave about the
 * sound. The bars now fill without a cue; a meter filling is not something that makes a noise.
 */
export const HomeMorph: React.FC<{gridStart?: number; turnAt?: number}> = ({gridStart = 0, turnAt}) => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);
  // Relative to the beat, not absolute: the bar grid re-times beats and a hard-coded frame 55 would
  // put the morph anywhere from mid-beat to past the end.
  const at = (f: number) => snap(gridStart + durationInFrames * f, 4) - gridStart;
  const FILL = at(0.10);
  const TURN = turnAt ?? at(0.62);
  const t = spring({frame: frame - TURN, fps, config: {damping: 200, stiffness: 34}});
  const fill = interpolate(frame, [FILL, TURN], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', width: '100%', height: '100%', alignItems: 'center'}}>
          <div style={{flex: '0 0 700px', paddingLeft: 110, display: 'flex', flexDirection: 'column', gap: 22}}>
            <Eyebrow delay={0}>Home</Eyebrow>
            <Title delay={4} size={72}>{'Home,\nredesigned'}</Title>
            <Body delay={12}>
              The logo is now a bell that shows what you missed. Start session stands out, with Plan
              beside it instead of buried three taps deep. The goal bars turned red. And the last tab
              stopped being you.
            </Body>
            {/* The same sound every seam makes: this is the version changing, drawn instead of
                filmed. Its swell peaks ten frames in, where the morph is halfway. */}
            <Cue at={TURN} sfx="sweep" gain={0.8} />
            <div style={{display: 'flex', gap: 18, marginTop: 10, alignItems: 'center'}}>
              <Era label="0.8.9" on={1 - t} />
              <div style={{width: 200, height: 3, borderRadius: 999, background: C.outline, position: 'relative'}}>
                <div style={{position: 'absolute', inset: 0, width: `${t * 100}%`, background: C.accent, borderRadius: 999}} />
              </div>
              <Era label="0.9" on={t} accent />
            </div>
          </div>
          <div style={{flex: 1, display: 'flex', justifyContent: 'center'}}>
            <div style={{transform: `translateY(${interpolate(t, [0, 1], [0, -8])}px)`}}>
              <Device height={940}>
                <HomeScreen width={940 / SHOT_AR} t={t} goalFill={fill} />
              </Device>
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
      opacity: 0.32 + on * 0.68, border: `1.5px solid ${accent ? C.accent : C.outline}`,
      borderRadius: 999, padding: '8px 20px', lineHeight: 1,
    }}
  >
    {label}
  </div>
);

/** The fifth tab changing hands, isolated from everything else. */
export const TabSwap: React.FC<{swapAt?: number}> = ({swapAt = 46}) => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);
  // The spring starts early enough that the handover's midpoint, not its first frame, sits on
  // `swapAt` — which the release cut puts on the downbeat the bed comes back on.
  const t = spring({frame: frame - (swapAt - LEAD - 4), fps, config: {damping: 200, stiffness: 44}});
  const out = Math.max(0, Math.min(1, (0.42 - t) / 0.42));
  const inn = Math.max(0, Math.min(1, (t - 0.58) / 0.42));
  const tabs = [
    {label: 'Cardio', icon: 'cardio' as const},
    {label: 'Stats', icon: 'stats' as const},
    {label: 'Home', icon: 'home' as const},
    {label: 'Coach', icon: 'coach' as const},
  ];
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {/* the tab actually changing hands. 0.7, not 0.95: this lands on the downbeat the bed comes
            back on at full strength, and the two summed put the loudest sample of the film here. */}
        <Cue at={swapAt} sfx="reveal" gain={0.7} />
        <div style={{display: 'flex', flexDirection: 'column', gap: 60, alignItems: 'center'}}>
          <div style={{display: 'flex', flexDirection: 'column', gap: 16, alignItems: 'center'}}>
            <Eyebrow delay={0}>The fifth tab</Eyebrow>
            <Title delay={4} size={78}>Academy gets a tab</Title>
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
            {/* The two tabs share one slot, so they must never both be legible: a 50/50 crossfade
                renders the words "Profile" and "Academy" on top of each other. The old one leaves
                before the new one arrives. */}
            <div style={{position: 'relative', width: 128, height: 79}}>
              <div
                style={{
                  position: 'absolute', left: 0, right: 0, bottom: 0,
                  opacity: out, transform: `translateY(${(1 - out) * -20}px)`,
                }}
              >
                <Tab label="Profile" icon="profile" on={false} />
              </div>
              <div
                style={{
                  position: 'absolute', left: 0, right: 0, bottom: 0,
                  opacity: inn, transform: `translateY(${(1 - inn) * 20}px)`,
                }}
              >
                <Tab label="Academy" icon="academy" on={false} badge />
              </div>
            </div>
          </div>
          <div style={{fontFamily: 'Georgia, serif', fontSize: 30, fontStyle: 'italic', color: C.muted, maxWidth: 900, textAlign: 'center'}}>
            The Academy is new in 0.9, so it needed somewhere to live. Profile moved up to the top of
            Home and gave up the fifth slot.
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
            <Title delay={4} size={74}>{'Leave the phone\nin your bag'}</Title>
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
