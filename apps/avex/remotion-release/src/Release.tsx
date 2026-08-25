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
 * Four things govern this edit:
 *
 *   Cut to the bar.  A beat is defined by the bar it ENDS on, never by a duration in seconds. The
 *                    grid is measured off the bed itself (see `BED` in theme.ts), and the lengths
 *                    below are derived so that a hard cut lands exactly on a downbeat and a
 *                    dissolve straddles one. That is the whole difference between a film that has
 *                    music and a film that is cut to it.
 *
 *   Say it plainly.  An earlier cut wrote headlines like "It shows its working" and "The coach, not
 *                    a footnote to it". They scan as clever and land as nothing — and the second one
 *                    is not even true, because the Academy is a place to read, not the coach.
 *
 *   One device.      Before and after are the same phone, never two. A side-by-side halves both
 *                    screens and turns a release into spot-the-difference; here 0.9 arrives *over*
 *                    0.8.9 under an accent edge — AccentRed replacing AccentNavy being, literally,
 *                    what shipped. The seam is timed to start on a downbeat too.
 *
 *   Never crop the   The full-bleed close-ups are gone. Cropping a capture to the frame edge put
 *   subject.         copy on top of live UI text and sliced headlines in half; a phone that stays a
 *                    phone, with the copy beside it, is worth more than the extra legibility.
 *
 * The one deliberate sync: the onboarding beat's hit lands on bar 33, which is where the bed cuts
 * away and comes back at full strength. The film's biggest moment and the track's are the same
 * moment.
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
  /* ── open ──────────────────────────────────── bed is near-silent until bar 5 ── */
  {
    endBar: 4, out: 'dissolve', shot: {z: [1.07, 1], ease: EASE.glide},
    el: () => <Card eyebrow="What's new" title="Avex 0.9" sub="147 commits · 632 files" />,
  },

  /* ── the day ────────────────────────────────────────────── bed enters, bar 5 ── */
  {endBar: 7, out: 'whip', shot: {z: [1, 1.05], x: [0, -22]}, el: () => <HomeMorph />},
  {endBar: 9, out: 'push', shot: {z: [1.04, 1], x: [26, 0]}, el: () => <NoticeBeat />},

  /* ── the session ─────────────────────────────────── bed opens up at bar 17 ── */
  {
    endBar: 13, out: 'cut', shot: {z: [1, 1.06], x: [0, -18], ease: EASE.drift},
    el: (start) => (
      <Compare
        before={{src: 'cfr/before-session.mp4', start: 690}}
        after={{src: 'cfr/after-session.mp4', start: 890}}
        eyebrow="Your workout" title={'The screen you\ntrain on'}
        note="The exercise name used to take up half the screen. Now your last session and what to lift next arrive together, the rest timer runs on its own, and one button finishes an exercise."
        hold={bar(12) - start} sweep={30} dir="ltr" height={880}
      />
    ),
  },
  {
    endBar: 15, out: 'push',
    el: () => (
      <Solo
        clip={{src: 'cfr/after-session.mp4', start: 950}}
        eyebrow="Your workout" title="It tells you what to lift"
        line="The weight to try next, and why. What you beat last time, right on the row. How many reps for a personal record."
        height={900} flip
      />
    ),
  },
  {endBar: 19, out: 'whip', shot: {z: [1.03, 1], y: [-12, 0], ease: EASE.drift}, el: () => <WatchBeat />},

  /* ── the record ─────────────────────────────── bed drops back at bar 25 ── */
  {
    endBar: 21, out: 'cut', shot: {z: [1, 1.05], x: [0, 20], ease: EASE.drift},
    el: (start) => (
      <Compare
        before={{src: 'cfr/before-coach.mp4', start: 107}}
        after={{src: 'cfr/after-coach.mp4', start: 179}}
        eyebrow="Coach" title="Coach explains itself"
        note="Your week, how recovered you are, and where you are in your training block — all on one page. And the volume number no longer gets cut off halfway through."
        hold={bar(20) - start + 30} sweep={30} dir="ltr" height={880} flip
      />
    ),
  },
  {
    endBar: 23, out: 'whip',
    el: () => (
      <Solo
        clip={{src: 'cfr/after-coach.mp4', start: 690}}
        eyebrow="New in 0.9" title="Training blocks"
        line="Build up, push hard, peak, then take a lighter week. You can start one straight from your weekly brief."
        height={900}
      />
    ),
  },
  {
    endBar: 26, out: 'cut', shot: {z: [1.05, 1], ease: EASE.drift},
    el: (start) => (
      <Compare
        before={{src: 'cfr/before-cardio.mp4', start: 31}}
        after={{src: 'cfr/after-cardio.mp4', start: 263}}
        eyebrow="Cardio" title="Built around your week"
        line="Two views instead of one long list — this week at a glance, or how you're trending over time."
        hold={bar(25) - start} sweep={28} dir="ttb" height={880} flip
      />
    ),
  },
  {
    endBar: 28, out: 'pushUp',
    el: () => (
      <Solo
        clip={{src: 'cfr/after-cardio.mp4', start: 880}}
        eyebrow="New in 0.9" title="Every week as a chart"
        line="One bar per week, back to your first, against the 150 minutes a week the WHO recommends."
        height={900}
      />
    ),
  },
  {
    endBar: 30, out: 'push', shot: {z: [1, 1.05], y: [8, -10]},
    el: () => (
      <Solo
        clip={{src: 'cfr/after-cardio.mp4', start: 1655}}
        eyebrow="History" title="Search everything you've done"
        line="Lifting and cardio in one list. Filter by how long it took, or how hard it was."
        height={900} flip
      />
    ),
  },
  {
    endBar: 32, out: 'dissolve', shot: {z: [1, 1.04], x: [0, 14], ease: EASE.drift},
    el: (start) => (
      <Compare
        before={{src: 'cfr/before-lasttab.mp4', start: 30}}
        after={{src: 'cfr/after-profile.mp4', start: 72}}
        eyebrow="Profile" title="Easier to read"
        line="Your year used to be twelve rows of dots. Now it's a month you can actually read — and the photo gallery is open to everyone."
        hold={bar(31) - start} sweep={30} dir="ltr" height={880}
      />
    ),
  },

  /* ── the shape of it ───── bar 32 is where the bed cuts away; 33 is the return ── */
  {
    endBar: 36, out: 'dissolve',
    el: (start) => (
      <OnboardingBeat
        before={ONBOARDING_0_8_9} after={ONBOARDING_0_9}
        eyebrow="Setting up"
        title="Fifteen questions down to nine"
        line="It used to ask fifteen things before showing you anything. Now it asks only what shapes your plan."
        /* the hit lands on bar 33 — the frame the bed comes back at full strength */
        phase={{bam: bar(33) - start}}
      />
    ),
  },
  {endBar: 38, out: 'cut', shot: {z: [1.03, 1]}, el: () => <TabSwap />},
  {
    endBar: 40, out: 'push', shot: {z: [1.02, 1.07], x: [10, -14], ease: EASE.drift},
    el: () => (
      <Solo
        clip={{src: 'cfr/after-academy.mp4', start: 40}}
        eyebrow="Academy" title={'Learn why\nyou’re doing it'}
        note="Thirty-five short reads on how training actually works — sets and reps, what a program really is, why the order matters. All of it open from the day you install. No levels, no unlocking, no progress bar."
        height={900} flip
      />
    ),
  },
  {
    endBar: 43, out: 'dissolve',
    el: () => (
      <ListCard
        eyebrow="Also in 0.9" title="Smaller things"
        items={[
          'Recovery is now called Wearable, with all three watch options on one row',
          'Tag a workout as a deload, a test, or technique work',
          'Filter your exercises, and your gear',
          'Make a custom exercise right from the search box',
          'Hold Start to skip the warm-up',
          'History groups by day — today, yesterday, Saturday',
          'The photo gallery is open to everyone',
          'Ember, a warmer accent colour if red isn’t for you',
        ]}
      />
    ),
  },
  {endBar: 46, shot: {z: [1, 1.04]}, el: () => <Card eyebrow="Avex" title="0.9" sub="out now" />},
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

const LAYOUT = BEATS.map((b, i) => ({beat: b, len: 0, start: 0}));
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
 * Under the picture, not over it. At 0.5 the whole film measured -20.3 LUFS integrated, which is
 * quiet enough that a platform would have to normalise it up; 0.62 puts it near -18.6 with the peak a safe 1.8 dB under full scale, leaving the
 * cues, which are transients at -3 dBFS source, room to cut through the busiest bars.
 */
const BED_LEVEL = 0.62;
const bedVolume = (f: number) =>
  BED_LEVEL * interpolate(f, [0, 20, TOTAL - 18, TOTAL], [0, 1, 1, 0], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp',
  });

export const Release: React.FC = () => (
  <AbsoluteFill style={{backgroundColor: BG}}>
    <Audio src={staticFile('music/bed.mp3')} volume={bedVolume} />
    <TransitionSeries>
      {LAYOUT.flatMap(({beat, len, start}, i) => {
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
    <Mark total={TOTAL} lead={bar(3)} tail={TOTAL - bar(42)} />
  </AbsoluteFill>
);
