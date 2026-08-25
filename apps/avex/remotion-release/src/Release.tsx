import React from 'react';
import {AbsoluteFill, Audio, interpolate, staticFile} from 'remotion';
import {TransitionSeries} from '@remotion/transitions';
import {Card, Compare, ListCard, Solo} from './Beats';
import {Camera, EASE, Shot} from './Camera';
import {HomeMorph, TabSwap} from './NativeBeats';
import {WatchBeat} from './BeatWatch';
import {NoticeBeat} from './BeatNotice';
import {ONBOARDING_0_8_9, ONBOARDING_0_9, OnboardingBeat} from './Onboarding';
import {Mark} from './Layout';
import {Xit, overlap, transition} from './Transitions';
import {BG, bar} from './theme';

/**
 * Avex 0.9 — the release cut.
 *
 * Every clip is served from `public/cfr/`: screenrecord writes VARIABLE frame rate (the session
 * takes averaged ~56fps), so a frame index measured with ffmpeg meant a different instant to
 * Remotion, which counts composition frames at 30. The captures are transcoded to constant 30fps so
 * the two agree; only then do the offsets below land where they were measured.
 *
 * Five things govern this edit:
 *
 *   Cut to the bar.    A beat is declared by the bar it ENDS on, never by a duration in seconds. The
 *                      grid is measured off the bed itself (`BED` in theme.ts) and lengths are
 *                      derived so a hard cut lands exactly on a downbeat and a dissolve straddles
 *                      one.
 *
 *   Seam on stillness. Every capture was profiled frame by frame for motion, and each seam sits in a
 *                      window where BOTH clips are at rest. A version change that fires while the
 *                      old screen is still coasting from a scroll reads as a glitch. After the seam
 *                      the clip may move as much as it likes, and on Coach it is meant to.
 *
 *   Say it plainly.    No em dashes, no tricolons, no "not X but Y". Every line is what a person
 *                      would say out loud about the thing.
 *
 *   One device.        Before and after are the same phone, never two.
 *
 *   Keep it moving.    Every beat carries a camera, and the joins are pushes.
 *
 * The one deliberate sync: the fifth tab changing hands lands on bar 33, the frame the bed cuts away
 * and comes back at full strength.
 */

type Beat = {
  /** the bar this beat hands over on */
  endBar: number;
  out?: Xit;
  shot?: Shot;
  /** `start` is the beat's own first frame, so a beat can place its own events on the grid */
  el: (start: number) => React.ReactNode;
};

const BEATS: Beat[] = [
  /* ── open ─────────────────────── two bars; the bed is near-silent until bar 5 ── */
  {
    endBar: 3, out: 'dissolve', shot: {z: [1.06, 1], ease: EASE.glide},
    el: () => <Card eyebrow="What's new" title="Avex 0.9" count={[147, 632]} />,
  },

  /* ── the day ────────────────────────────────────────────── bed enters, bar 5 ── */
  {endBar: 6, out: 'whip', shot: {z: [1, 1.05], x: [0, -20]}, el: () => <HomeMorph />},
  {endBar: 8, out: 'push', shot: {z: [1.04, 1], x: [24, 0]}, el: () => <NoticeBeat />},

  /* ── the session ─────────────────────────────────── bed opens up at bar 17 ── */
  {
    endBar: 12, out: 'cut', shot: {z: [1, 1.05], x: [0, -16], ease: EASE.drift},
    el: (start) => (
      <Compare
        before={{src: 'cfr/before-session.mp4', start: 690}}
        after={{src: 'cfr/after-session.mp4', start: 890}}
        eyebrow="Your workout" title={'The screen you\ntrain on'}
        note="The exercise name used to take up half the screen. Now your last session and what to lift next show up together, the rest timer runs on its own, and one button finishes an exercise."
        hold={bar(9) - start} sweep={30} dir="ltr" height={880}
      />
    ),
  },
  {
    endBar: 14, out: 'push',
    el: () => (
      <Solo
        clip={{src: 'cfr/after-session.mp4', start: 950}}
        eyebrow="Your workout" title="It tells you what to lift"
        line="The weight to try next, and why. What you beat last time, right there on the row. How many reps you need for a personal record."
        height={900} flip changes={[8]}
      />
    ),
  },
  {endBar: 18, out: 'whip', shot: {z: [1.03, 1], y: [-10, 0], ease: EASE.drift}, el: () => <WatchBeat />},

  /* ── the record ────────────────────────────────── bed drops back at bar 25 ── */
  {
    /* Seven bars. Coach was the biggest rebuild in the release and was getting four seconds; the
       compare and the training-block beat are now one continuous shot, and the capture scrolls down
       to the block rail on its own once the seam has passed. */
    endBar: 25, out: 'cut', shot: {z: [1, 1.05], x: [0, 18], ease: EASE.drift},
    el: (start) => (
      <Compare
        /* The after clip tours the page: the seam happens at the top, where 0.8.9's volume figure
           reads "52...." against 0.9's "52.8k lb", and then it scrolls down to the block rail. 0.8x
           because the capture only travels from the header to the block over about 130 frames and
           then scrolls back up again; slowed, that stretch fills the whole beat. */
        before={{src: 'cfr/before-coach.mp4', start: 180}}
        after={{src: 'cfr/after-coach.mp4', start: 424, rate: 0.8}}
        eyebrow="Coach" title="Coach shows its work"
        note="Your week, your recovery, and where you are in a training block, all on one page. The volume figure stops getting cut off halfway through. And the block itself is new: build up, push hard, peak, then take a lighter week, started straight from the brief."
        hold={bar(19) - start} sweep={30} dir="ltr" height={880} flip
      />
    ),
  },
  {
    endBar: 28, out: 'cut', shot: {z: [1.04, 1], ease: EASE.drift},
    el: (start) => (
      <Compare
        before={{src: 'cfr/before-cardio.mp4', start: 0}}
        after={{src: 'cfr/after-cardio.mp4', start: 8}}
        eyebrow="Cardio" title="Built around your week"
        line="Two views instead of one long list. This week at a glance, or how you are trending over time."
        hold={bar(26) - start} sweep={24} dir="ttb" height={880}
      />
    ),
  },
  {
    endBar: 30, out: 'pushUp',
    el: () => (
      <Solo
        clip={{src: 'cfr/after-cardio.mp4', start: 880}}
        eyebrow="New in 0.9" title="Every week as a chart"
        line="One bar per week, back to your very first, against the 150 minutes a week the WHO recommends."
        height={900} changes={[8]} fill={22}
      />
    ),
  },
  {
    /* before-lasttab is only still for its first 89 frames, so this seam sits early rather than on
       the downbeat. A seam that fires while the old screen is still coasting looks broken. */
    endBar: 32, out: 'dissolve', shot: {z: [1, 1.04], x: [0, 12], ease: EASE.drift},
    el: () => (
      <Compare
        before={{src: 'cfr/before-lasttab.mp4', start: 4}}
        after={{src: 'cfr/after-profile.mp4', start: 6}}
        eyebrow="Profile" title="Easier to read"
        line="Your year used to be twelve rows of dots. Now it is a month you can actually read, and the photo gallery is open to everyone."
        hold={40} sweep={26} dir="ltr" height={880} flip
      />
    ),
  },

  /* ── the shape of it ────── bar 32 is where the bed cuts away, 33 is the return ── */
  {
    endBar: 34, out: 'cut', shot: {z: [1.03, 1]},
    el: (start) => <TabSwap swapAt={bar(33) - start} />,
  },
  {
    endBar: 37, out: 'push', shot: {z: [1.02, 1.07], x: [8, -12], ease: EASE.drift},
    el: () => (
      <Solo
        clip={{src: 'cfr/after-academy.mp4', start: 40}}
        eyebrow="Academy" title={'Learn why\nyou’re doing it'}
        note="Thirty-five short reads on how training actually works. Sets and reps, what a program really is, why the order you do things in matters. All of it open from the day you install, with nothing to unlock."
        height={900} flip changes={[10]}
      />
    ),
  },
  {
    endBar: 41, out: 'dissolve',
    el: () => (
      <OnboardingBeat
        before={ONBOARDING_0_8_9} after={ONBOARDING_0_9}
        eyebrow="Setting up"
        title="Fifteen questions down to nine"
        line="It used to ask fifteen things before showing you anything. Now it asks what shapes your plan, and the rest waits until the end."
      />
    ),
  },
  {
    endBar: 44, out: 'dissolve',
    el: () => (
      <ListCard
        eyebrow="Also in 0.9" title="Smaller things"
        items={[
          'Recovery is now called Wearable, with all three watch options on one row',
          'Tag a workout as a deload, a test, or technique work',
          'Filters for your exercises and for your gear',
          'Make a custom exercise right from the search box',
          'Hold Start to skip the warm-up',
          'History groups by day: today, yesterday, Saturday',
          'The photo gallery is open to everyone',
          'Ember, a warmer accent colour if red is not for you',
        ]}
      />
    ),
  },
  {
    endBar: 46, shot: {z: [1, 1.035]},
    el: (start) => <Card eyebrow="Avex" title="0.9" sub="out now" subAt={bar(45) - start} />,
  },
];

/* ── laying the beats on the grid ────────────────────────────────────────── */

/**
 * A transition of T frames overlaps the two sequences it joins by T, so a beat's length is not
 * simply the gap between downbeats. For the *visible* handover to sit on bar `endBar`, a beat has to
 * run half a transition past it and the next has to start half a transition early:
 *
 *   len[i] = bar(endBar[i]) - bar(endBar[i-1]) + T[i-1]/2 + T[i]/2
 *
 * With a hard cut both halves are zero and the cut lands exactly on the downbeat.
 */
const T = (b: Beat) => overlap(b.out ?? 'fade');

const LAYOUT = BEATS.map(() => ({len: 0, start: 0}));
{
  let prevEnd = 0;
  let clock = 0;
  for (let i = 0; i < BEATS.length; i++) {
    const end = bar(BEATS[i].endBar);
    const tPrev = i === 0 ? 0 : T(BEATS[i - 1]);
    const tNext = i === BEATS.length - 1 ? 0 : T(BEATS[i]);
    LAYOUT[i].len = end - prevEnd + tPrev / 2 + tNext / 2;
    LAYOUT[i].start = clock;
    clock += LAYOUT[i].len - tNext;
    prevEnd = end;
  }
}

export const TOTAL = LAYOUT[LAYOUT.length - 1].start + LAYOUT[LAYOUT.length - 1].len;

/* ── the bed ─────────────────────────────────────────────────────────────── */

/**
 * Under the picture, not over it.
 *
 * The bed's last third sat on top of everything, so this rides the fader the way a mix engineer
 * would: the return at bar 33 lands at full strength, then settles over the following bars.
 *
 * The first attempt rode it to 0.6 and overshot — measured on the finished mix, the climax came out
 * 1.7 dB QUIETER than the middle of the film, which is worse than the problem it was fixing. At 0.78
 * the climax sits about half a decibel above the mid-section: still the loudest thing in the film,
 * no longer shouting over it.
 */
const BED_LEVEL = 0.72;
const RETURN = bar(33);
const SETTLED = bar(36);

const bedRide = (f: number) =>
  interpolate(f, [0, RETURN - 2, RETURN, SETTLED, TOTAL], [1, 1, 1, 0.78, 0.78], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });

const bedVolume = (f: number) =>
  BED_LEVEL * bedRide(f) *
  interpolate(f, [0, 16, TOTAL - 16, TOTAL], [0, 1, 1, 0], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });

export const Release: React.FC = () => (
  <AbsoluteFill style={{backgroundColor: BG}}>
    <Audio src={staticFile('music/bed.mp3')} volume={bedVolume} />
    <TransitionSeries>
      {BEATS.flatMap((beat, i) => {
        const {len, start} = LAYOUT[i];
        const seq = (
          <TransitionSeries.Sequence key={`s${i}`} durationInFrames={Math.round(len)}>
            <Camera shot={beat.shot}>{beat.el(start)}</Camera>
          </TransitionSeries.Sequence>
        );
        if (i === BEATS.length - 1) return [seq];
        const t = transition(beat.out ?? 'fade', `t${i}`);
        return t ? [seq, t] : [seq];
      })}
    </TransitionSeries>
    <Mark total={TOTAL} lead={bar(2)} tail={TOTAL - bar(43)} />
  </AbsoluteFill>
);
