import React from 'react';
import {AbsoluteFill} from 'remotion';
import {TransitionSeries} from '@remotion/transitions';
import {Card, Compare, Detail, ListCard, Solo} from './Beats';
import {Camera, EASE, Shot} from './Camera';
import {HomeMorph, TabSwap} from './NativeBeats';
import {WatchBeat} from './BeatWatch';
import {NoticeBeat} from './BeatNotice';
import {ONBOARDING_0_8_9, ONBOARDING_0_9, OnboardingBeat} from './Onboarding';
import {Mark} from './Layout';
import {Xit, overlap, transition} from './Transitions';
import {BG, SEC} from './theme';

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
 *   Say it plainly.  An earlier cut wrote headlines like "It shows its working" and "The coach, not
 *                    a footnote to it". They scan as clever and land as nothing — and the second one
 *                    is not even true, because the Academy is a place to read, not the coach. Every
 *                    line here says what the thing does, in the words a person would use out loud.
 *
 *   Pace.            Eighteen beats in ninety seconds cannot all be the same length. A beat that
 *                    holds a paragraph gets six or seven seconds; a beat that holds one line gets
 *                    four. `XFADE.cut` is a genuine zero-frame cut, because an edit whose every join
 *                    is a crossfade has no punctuation, only commas.
 *
 *   One device.      Before and after are the same phone, never two. A side-by-side halves both
 *                    screens and turns a release into spot-the-difference; here 0.9 arrives *over*
 *                    0.8.9 under an accent edge — AccentRed replacing AccentNavy being, literally,
 *                    what shipped.
 *
 *   Keep it moving.  Every beat carries a camera. `shot` moves the whole frame; `focus` (on Detail)
 *                    flies a window inside the capture. A held frame in a ninety-second film reads
 *                    as a slide, and the joins are pushes for the same reason — the camera should
 *                    seem to travel from one subject to the next, not cut a graphic over another.
 */

type Beat = {len: number; el: React.ReactNode; out?: Xit; shot?: Shot};

export const BEATS: Beat[] = [
  /* ── open ────────────────────────────────────────────────────────────── */
  {
    len: SEC(5), out: 'dissolve', shot: {z: [1.07, 1], ease: EASE.glide},
    el: <Card eyebrow="What's new" title="Avex 0.9" sub="147 commits · 632 files" />,
  },

  /* ── the day ─────────────────────────────────────────────────────────── */
  {len: SEC(7), out: 'whip', shot: {z: [1, 1.05], x: [0, -22]}, el: <HomeMorph />},
  {
    len: SEC(4), out: 'cut',
    el: (
      <Detail
        clip={{src: 'cfr/after-home.mp4', start: 20}}
        eyebrow="Home" title="Everything before you scroll"
        line="How long it's been, what you're lifting today and at what weight, and one button to start."
        focus={{from: {y: 0.10, z: 1.0}, to: {y: 0.31, z: 1.07}}}
      />
    ),
  },
  {len: SEC(5), out: 'push', shot: {z: [1.04, 1], x: [26, 0]}, el: <NoticeBeat />},

  /* ── the session ─────────────────────────────────────────────────────── */
  {
    len: SEC(7), out: 'cut', shot: {z: [1, 1.06], x: [0, -18], ease: EASE.drift},
    el: (
      <Compare
        before={{src: 'cfr/before-session.mp4', start: 690}}
        after={{src: 'cfr/after-session.mp4', start: 890}}
        eyebrow="Your workout" title={'The screen you\ntrain on'}
        note="The exercise name used to take up half the screen. Now your last session and what to lift next arrive together, the rest timer runs on its own, and one button finishes an exercise."
        hold={62} sweep={30} dir="ltr" height={880}
      />
    ),
  },
  {
    len: SEC(4.5), out: 'push',
    el: (
      <Detail
        clip={{src: 'cfr/after-session.mp4', start: 950}}
        eyebrow="Your workout" title="It tells you what to lift"
        line="The weight to try next, and why. What you beat last time, right on the row. How many reps you need for a personal record."
        focus={{from: {y: 0.17, z: 1.0}, to: {y: 0.48, z: 1.06}}}
      />
    ),
  },
  {len: SEC(7.5), out: 'whip', shot: {z: [1.03, 1], y: [-12, 0], ease: EASE.drift}, el: <WatchBeat />},

  /* ── the record ──────────────────────────────────────────────────────── */
  {
    len: SEC(6.5), out: 'cut', shot: {z: [1, 1.05], x: [0, 20], ease: EASE.drift},
    el: (
      <Compare
        before={{src: 'cfr/before-coach.mp4', start: 107}}
        after={{src: 'cfr/after-coach.mp4', start: 179}}
        eyebrow="Coach" title="Coach explains itself"
        note="Your week, how recovered you are, and where you are in your training block — all on one page. And the volume number no longer gets cut off halfway through."
        hold={56} sweep={30} dir="ltr" height={880} flip
      />
    ),
  },
  {
    len: SEC(4), out: 'whip',
    el: (
      <Detail
        clip={{src: 'cfr/after-coach.mp4', start: 690}}
        eyebrow="New in 0.9" title="Training blocks"
        line="Build up, push hard, peak, then take a lighter week. You can start one straight from your weekly brief."
        focus={{from: {y: 0.40, z: 1.0}, to: {y: 0.65, z: 1.06}}}
      />
    ),
  },
  {
    len: SEC(5.5), out: 'cut', shot: {z: [1.05, 1], ease: EASE.drift},
    el: (
      <Compare
        before={{src: 'cfr/before-cardio.mp4', start: 31}}
        after={{src: 'cfr/after-cardio.mp4', start: 263}}
        eyebrow="Cardio" title="Built around your week"
        line="Two views instead of one long list — this week at a glance, or how you're trending over time."
        hold={48} sweep={28} dir="ttb" height={880}
      />
    ),
  },
  {
    len: SEC(4), out: 'pushUp',
    el: (
      <Detail
        clip={{src: 'cfr/after-cardio.mp4', start: 880}}
        eyebrow="New in 0.9" title="Every week as a chart"
        line="One bar per week, all the way back to your first, against the 150 minutes a week the WHO recommends."
        focus={{from: {y: 0.20, z: 1.0}, to: {y: 0.47, z: 1.05}}}
      />
    ),
  },
  {
    len: SEC(3.5), out: 'push', shot: {z: [1, 1.06], y: [8, -10]},
    el: (
      <Solo
        clip={{src: 'cfr/after-cardio.mp4', start: 1655}}
        eyebrow="History" title="Search everything you've done"
        line="Lifting and cardio in one list. Filter by how long it took, or how hard it was."
        height={900}
      />
    ),
  },
  {
    len: SEC(5.5), out: 'dissolve', shot: {z: [1, 1.05], x: [0, 18], ease: EASE.drift},
    el: (
      <Compare
        before={{src: 'cfr/before-lasttab.mp4', start: 30}}
        after={{src: 'cfr/after-profile.mp4', start: 72}}
        eyebrow="Profile" title="Easier to read"
        line="Your year used to be twelve rows of dots. Now it's a month you can actually read — and the photo gallery is open to everyone."
        hold={48} sweep={30} dir="ltr" height={880} flip
      />
    ),
  },

  /* ── the shape of it ─────────────────────────────────────────────────── */
  {len: SEC(3.5), out: 'cut', shot: {z: [1.03, 1]}, el: <TabSwap />},
  {
    len: SEC(5), out: 'push', shot: {z: [1.02, 1.08], x: [10, -14], ease: EASE.drift},
    el: (
      <Solo
        clip={{src: 'cfr/after-academy.mp4', start: 40}}
        eyebrow="Academy" title={'Learn why\nyou’re doing it'}
        note="Thirty-five short reads on how training actually works — sets and reps, what a program really is, why the order matters. All of it open from the day you install. No levels, no unlocking, no progress bar."
        height={900} flip
      />
    ),
  },
  {
    len: SEC(8), out: 'dissolve',
    el: (
      <OnboardingBeat
        before={ONBOARDING_0_8_9} after={ONBOARDING_0_9}
        eyebrow="Setting up"
        title="Fifteen questions down to nine"
        line="It used to ask fifteen things before showing you anything. Now it asks only what shapes your plan."
      />
    ),
  },
  {
    len: SEC(6), out: 'dissolve',
    el: (
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
  {len: SEC(4.5), shot: {z: [1, 1.05]}, el: <Card eyebrow="Avex" title="0.9" sub="out now" />},
];

/* ── assembly ────────────────────────────────────────────────────────────── */

/**
 * A transition of T frames overlaps the two sequences it joins by T, so the running time is not the
 * sum of the beats — it is that sum less every join. Computing it rather than hard-coding a duration
 * means re-timing a beat cannot silently leave a black tail on the end of the film.
 */
export const TOTAL = BEATS.reduce(
  (n, b, i) => n + b.len - (i < BEATS.length - 1 ? overlap(b.out ?? 'fade') : 0),
  0
);

export const Release: React.FC = () => (
  <AbsoluteFill style={{backgroundColor: BG}}>
    <TransitionSeries>
      {BEATS.flatMap((b, i) => {
        const seq = (
          <TransitionSeries.Sequence key={`s${i}`} durationInFrames={b.len}>
            <Camera shot={b.shot}>{b.el}</Camera>
          </TransitionSeries.Sequence>
        );
        if (i === BEATS.length - 1) return [seq];
        const t = transition(b.out ?? 'fade', `t${i}`);
        return t ? [seq, t] : [seq];
      })}
    </TransitionSeries>
    <Mark total={TOTAL} lead={SEC(4.4)} tail={SEC(5)} />
  </AbsoluteFill>
);
