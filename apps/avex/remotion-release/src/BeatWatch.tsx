import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {WatchPr, WatchRest, WatchRpe, WatchSet} from './ui/Watch';
import {C} from './ui/tokens';
import {Plate, Split} from './Layout';
import {Cue, CueRun} from './Sound';
import {Body, Eyebrow, Title, useEdgeFade, useRise} from './Type';
import {ACCENT, MONO, MUTED, ON_BG, SERIF} from './theme';

/**
 * Wear OS. The only genuinely new *surface* in 0.9 — neither `wear/` nor `shared/` existed at
 * 0.8.9 — so it gets the longest hold in the film and the only sound design with a narrative:
 * the stepper ticks, the set logs, the record answers in gold, the rest runs.
 *
 * The flow is the watch's own (§16, one decision per screen): adjust → log → PR → rest → rate.
 * Nothing here is a screen recording; the watch is drawn from WearTheme.kt and SessionScreen.kt,
 * which is the only way to get a 454px round display legible at 1080p.
 */

type Step = {label: string; n: string};

const STEPS: Step[] = [
  {n: '01', label: 'Adjust and log, on the wrist'},
  {n: '02', label: 'A record answers in gold'},
  {n: '03', label: 'The rest is kept for you'},
  {n: '04', label: 'Rate it, or don’t'},
];

export const WatchBeat: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);

  // Phase boundaries as fractions of the beat, so re-timing the cut does not desync the sound.
  const at = (f: number) => Math.round(durationInFrames * f);
  const P = {set: 0, log: at(0.34), pr: at(0.38), rest: at(0.48), rpe: at(0.76)};

  // The ± stepper walking 150 → 155, one tick per step.
  const stepP = interpolate(frame, [at(0.1), at(0.3)], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });
  const weight = 150 + Math.round(stepP * 5);

  const enter = (f: number) => spring({frame: frame - f, fps, config: {damping: 200, stiffness: 110}});
  const ePr = enter(P.pr) * (1 - enter(P.rest));
  const eRest = enter(P.rest) * (1 - enter(P.rpe));
  const eRpe = enter(P.rpe);
  const eSet = 1 - enter(P.pr);

  const idx = frame >= P.rpe ? 3 : frame >= P.rest ? 2 : frame >= P.pr ? 1 : 0;
  const step = STEPS[idx];

  const restT = interpolate(frame, [P.rest, P.rpe], [180, 121], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {/* the stepper, the log, the double-tick a PR answers with, then the rest starting */}
        <CueRun from={at(0.11)} every={(at(0.3) - at(0.1)) / 5} count={5} sfx="tick" gain={0.85} />
        <Cue at={P.log} sfx="confirm" />
        <Cue at={P.pr + 3} sfx="tick" gain={1.1} />
        <Cue at={P.pr + 10} sfx="tick" gain={1.1} />
        <Cue at={P.rest + 2} sfx="restStart" />
        <Cue at={P.rpe + 4} sfx="tap" gain={0.8} />

        <Split
          copyWidth={700}
          copy={
            <>
              <Eyebrow delay={0}>New in 0.9 · Wear OS</Eyebrow>
              <Title delay={4} size={72}>{'Leave the phone\nin your bag'}</Title>
              <Body delay={10} width={620}>
                A companion app that mirrors the live session: turn the bezel to change the load, log
                the set, and the rest timer runs on your wrist. Heart rate rides along, and it writes
                back through Health Connect.
              </Body>
              <StepLine step={step} />
              <Chips items={['2 tiles', '3 complications', 'Health Connect']} delay={22} />
            </>
          }
        >
          <div style={{position: 'relative', width: 560, height: 620, display: 'flex', justifyContent: 'center'}}>
            <Layer on={eSet}><WatchSet size={430} weight={String(weight)} reps="9" /></Layer>
            <Layer on={ePr}><WatchPr size={430} value="155" on={ePr} /></Layer>
            <Layer on={eRest}><WatchRest size={430} remaining={restT} total={210} /></Layer>
            <Layer on={eRpe}><WatchRpe size={430} rpe={8.5} /></Layer>
          </div>
        </Split>
      </Plate>
    </AbsoluteFill>
  );
};

const StepLine: React.FC<{step: Step}> = ({step}) => (
  <div style={{marginTop: 14, display: 'flex', gap: 14, alignItems: 'baseline', minHeight: 42}}>
    <span style={{fontFamily: MONO, fontSize: 18, color: ACCENT, letterSpacing: 2}}>{step.n}</span>
    <span style={{fontFamily: SERIF, fontSize: 31, color: ON_BG}}>{step.label}</span>
  </div>
);

const Chips: React.FC<{items: string[]; delay: number}> = ({items, delay}) => (
  <div style={{display: 'flex', gap: 12, marginTop: 4}}>
    {items.map((s, i) => (
      <Chip key={i} label={s} delay={delay + i * 3} />
    ))}
  </div>
);

const Chip: React.FC<{label: string; delay: number}> = ({label, delay}) => (
  <div
    style={{
      fontFamily: MONO, fontSize: 15, letterSpacing: 1.6, textTransform: 'uppercase', color: MUTED,
      border: `1px solid ${C.outline}`, borderRadius: 999, padding: '7px 15px', ...useRise(delay, 8),
    }}
  >
    {label}
  </div>
);

const Layer: React.FC<{on: number; children: React.ReactNode}> = ({on, children}) => (
  <div
    style={{
      position: 'absolute', top: '50%', left: '50%',
      transform: `translate(-50%,-50%) scale(${0.93 + on * 0.07})`,
      opacity: on, pointerEvents: 'none',
    }}
  >
    {children}
  </div>
);
