import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {WatchPr, WatchRest, WatchRpe, WatchSet} from './ui/Watch';
import {C} from './ui/tokens';
import {EASE} from './Camera';
import {Plate, Split} from './Layout';
import {Cue} from './Sound';
import {Body, Eyebrow, Title, useEdgeFade, useRise} from './Type';
import {ACCENT, EIGHTH, MONO, MUTED, ON_BG, SERIF, run, snap} from './theme';

/**
 * Wear OS. The only genuinely new *surface* in 0.9 — neither `wear/` nor `shared/` existed at
 * 0.8.9 — so it gets the longest hold in the film.
 *
 * The flow is the watch's own (§16, one decision per screen): adjust → log → PR → rest → rate.
 * Nothing here is a screen recording; the watch is drawn from WearTheme.kt and SessionScreen.kt,
 * which is the only way to get a 454px round display legible at 1080p.
 */

type Step = {label: string; n: string};

const STEPS: Step[] = [
  {n: '01', label: 'Set the weight and log it'},
  {n: '02', label: 'Beat your record and it says so'},
  {n: '03', label: 'Rest counts down on your wrist'},
  {n: '04', label: 'Say how hard it felt, or skip it'},
];

export const WatchBeat: React.FC<{gridStart?: number}> = ({gridStart = 0}) => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);

  // Phase boundaries as fractions of the beat, so re-timing the cut does not desync the sound.
  // The beat is three bars in the release cut (it was four): the five ticks are done by 0.31, the
  // set logs at 0.33, the PR screen keeps most of a second, and the rest timer runs out ON a grid
  // point, because the chime that says so has to.
  const at = (f: number) => Math.round(durationInFrames * f);
  const P = {set: 0, log: at(0.33), pr: at(0.36), rest: at(0.47), rpe: snap(gridStart + at(0.76), 4) - gridStart};

  // The stepper walks 150 → 155 on eighth notes, and the picture is driven BY the cue frames rather
  // than the other way round, so the number changing and the tick are the same instant.
  const ticks = run(gridStart + at(0.14), 5, EIGHTH).map((f) => f - gridStart);
  const weight = 150 + ticks.filter((f) => frame >= f).length;

  const enter = (f: number) => spring({frame: frame - f, fps, config: {damping: 200, stiffness: 110}});
  const ePr = enter(P.pr) * (1 - enter(P.rest));
  const eRest = enter(P.rest) * (1 - enter(P.rpe));
  const eRpe = enter(P.rpe);
  const eSet = 1 - enter(P.pr);

  const idx = frame >= P.rpe ? 3 : frame >= P.rest ? 2 : frame >= P.pr ? 1 : 0;
  const step = STEPS[idx];

  // The ring drains to 0:00 on the frame the rate screen takes over. Five seconds in 1.7 s: the
  // countdown is a device, not a claim, and the previous cut ran 3:00 → 2:01 in the same time.
  const restT = interpolate(frame, [P.rest + 4, P.rpe], [5, 0], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });

  // One light source walking across the case for the length of the beat. A watch drawn as a flat
  // black disc reads as an icon; a moving specular band is what makes it read as a thing.
  const light = interpolate(frame, [0, durationInFrames], [0.08, 0.92], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.drift,
  });

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {/* The stepper walking the weight up is the one sound on this beat that reads as the watch
            rather than as stock UI, so it is the one that stays. The confirm/rest chimes are cut. */}
        {ticks.map((f, i) => <Cue key={i} at={f} sfx="tick" gain={0.95} />)}
        {/* time's up: the three rising pips synthesised for exactly this, on the frame the ring
            reaches empty */}
        <Cue at={P.rpe} sfx="restDone" gain={0.9} />

        <Split
          copyWidth={700}
          copy={
            <>
              <Eyebrow delay={0}>New in 0.9 · Wear OS</Eyebrow>
              <Title delay={4} size={72}>{'Leave your phone\nin your bag'}</Title>
              <Body delay={10} width={620}>
                Avex runs on your watch now. Turn the bezel to change the weight, tap once to log the
                set, and the rest timer counts down on your wrist. It reads your heart rate while you
                lift and saves everything back through Health Connect.
              </Body>
              <StepLine step={step} />
              <Chips items={['2 tiles', '3 complications', 'Health Connect']} delay={22} />
            </>
          }
        >
          <div style={{position: 'relative', width: 600, height: 660, display: 'flex', justifyContent: 'center'}}>
            <Layer on={eSet}><WatchSet size={468} weight={String(weight)} reps="9" light={light} /></Layer>
            <Layer on={ePr}><WatchPr size={468} value="155" on={ePr} light={light} /></Layer>
            <Layer on={eRest}><WatchRest size={468} remaining={restT} total={15} light={light} /></Layer>
            <Layer on={eRpe}><WatchRpe size={468} rpe={8.5} light={light} /></Layer>
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
