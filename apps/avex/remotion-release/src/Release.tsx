import React from 'react';
import {AbsoluteFill} from 'remotion';
import {TransitionSeries} from '@remotion/transitions';
import {Card, Compare, Detail, ListCard, Solo} from './Beats';
import {GoalCloseUp, HomeMorph, TabSwap} from './NativeBeats';
import {WatchBeat} from './BeatWatch';
import {NoticeBeat} from './BeatNotice';
import {ONBOARDING_0_8_9, ONBOARDING_0_9, OnboardingBeat} from './Onboarding';
import {Act, Rail} from './Layout';
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
 * Three things govern this edit:
 *
 *   Pace.       Nineteen beats in ninety seconds cannot all be the same length. A beat that holds a
 *               paragraph gets six or seven seconds; a beat that holds one line gets four; a beat
 *               that holds a headline gets three, and is cut hard on both sides.
 *
 *   One device. Before and after are the same phone, not two. A side-by-side halves both screens and
 *               turns a release into spot-the-difference; here 0.9 arrives *over* 0.8.9 under an
 *               accent edge — AccentRed replacing AccentNavy being, literally, what shipped.
 *
 *   Real claims. Every figure below is on screen in the footage or asserted by the source: 147
 *               commits and 632 files across `2fe2379..main`; 35 pieces in Academy (its own header);
 *               1,486.9k lb lifetime (Profile); fifteen onboarding pages down to nine (the 0.9
 *               dispatch states the nine itself). An earlier cut said "28 lessons" and "17 pages
 *               down to 10"; neither survives contact with the code.
 */

type Beat = {len: number; el: React.ReactNode; out?: Xit; act?: string};

export const BEATS: Beat[] = [
  /* ── open ────────────────────────────────────────────────────────────── */
  {
    act: 'Avex 0.9', len: SEC(5), out: 'sweep',
    el: <Card eyebrow="Release" title="Avex 0.9" sub="147 commits · 632 files" shot={{z: [1.06, 1]}} />,
  },

  /* ── the day ─────────────────────────────────────────────────────────── */
  {act: 'The day', len: SEC(7), out: 'whip', el: <HomeMorph />},
  {
    len: SEC(4), out: 'cut',
    el: (
      <Detail
        clip={{src: 'cfr/after-home.mp4', start: 20}}
        eyebrow="Home · in detail" title="The top of Home, rebuilt"
        line="A bell with your unread count, the days since your last session, today's lifts at their weights — and Plan split away from Start."
        focus={{from: {y: 0.10, z: 1.0}, to: {y: 0.31, z: 1.07}}}
      />
    ),
  },
  {len: SEC(4.5), out: 'whipUp', el: <NoticeBeat />},
  {len: SEC(3.5), out: 'dissolve', el: <GoalCloseUp />},

  /* ── the session ─────────────────────────────────────────────────────── */
  {
    act: 'The session', len: SEC(7), out: 'cut',
    el: (
      <Compare
        before={{src: 'cfr/before-session.mp4', start: 690}}
        after={{src: 'cfr/after-session.mp4', start: 890}}
        eyebrow="The session" title={'The screen you\nactually train on'}
        note="The title stops eating the fold. Last session and suggested next arrive in the same breath, the rest timer keeps its own time, and one button closes out an exercise."
        hold={62} sweep={26} dir="ltr" height={880}
      />
    ),
  },
  {
    len: SEC(4.5), out: 'sweep',
    el: (
      <Detail
        clip={{src: 'cfr/after-session.mp4', start: 950}}
        eyebrow="The session · in detail" title="It tells you what to do next"
        line="Suggested next, and why. What you beat, on the row. Reps left for the PR."
        focus={{from: {y: 0.17, z: 1.0}, to: {y: 0.48, z: 1.06}}}
      />
    ),
  },
  {len: SEC(7.5), out: 'whip', el: <WatchBeat />},

  /* ── the record ──────────────────────────────────────────────────────── */
  {
    act: 'The record', len: SEC(6.5), out: 'cut',
    el: (
      <Compare
        before={{src: 'cfr/before-coach.mp4', start: 107}}
        after={{src: 'cfr/after-coach.mp4', start: 179}}
        eyebrow="Coach" title="It shows its working"
        note="Signals, recovery load and the week's block in one place — and a volume figure that no longer truncates to 52…"
        hold={56} sweep={26} dir="ltr" height={880} flip
      />
    ),
  },
  {
    len: SEC(4), out: 'whip',
    el: (
      <Detail
        clip={{src: 'cfr/after-coach.mp4', start: 690}}
        eyebrow="Coach · new" title="A block, with its four phases"
        line="Accumulate, intensify, peak, deload — on one rail, startable from the brief."
        focus={{from: {y: 0.40, z: 1.0}, to: {y: 0.65, z: 1.06}}}
      />
    ),
  },
  {
    len: SEC(5.5), out: 'cut',
    el: (
      <Compare
        before={{src: 'cfr/before-cardio.mp4', start: 31}}
        after={{src: 'cfr/after-cardio.mp4', start: 263}}
        eyebrow="Cardio" title="One week, two lenses"
        line="Rebuilt around the week in front of you, with a WEEK and a PROGRESS lens instead of one long list."
        hold={48} sweep={24} dir="ttb" height={880}
      />
    ),
  },
  {
    len: SEC(3.5), out: 'whipUp',
    el: (
      <Detail
        clip={{src: 'cfr/after-cardio.mp4', start: 880}}
        eyebrow="Cardio · new" title="Weeks, drawn — not listed"
        line="One bar per week against the WHO 150-minute line, back to the first week you logged."
        focus={{from: {y: 0.20, z: 1.0}, to: {y: 0.47, z: 1.05}}}
      />
    ),
  },
  {
    len: SEC(3.5), out: 'sweep',
    el: (
      <Solo
        clip={{src: 'cfr/after-cardio.mp4', start: 1655}}
        eyebrow="History" title="Everything, searchable"
        line="One list across lifting and cardio, filterable by length and effort."
        height={900} shot={{z: [1.0, 1.07], y: [10, -14]}}
      />
    ),
  },
  {
    len: SEC(5.5), out: 'dissolve',
    el: (
      <Compare
        before={{src: 'cfr/before-lasttab.mp4', start: 30}}
        after={{src: 'cfr/after-profile.mp4', start: 72}}
        eyebrow="Profile" title="A year that fits on the page"
        line="The twelve-row year grid becomes a month you can read, and the gallery is open to everyone."
        hold={48} sweep={26} dir="ltr" height={880} flip
      />
    ),
  },

  /* ── the shape of it ─────────────────────────────────────────────────── */
  {act: 'The shape of it', len: SEC(3.5), out: 'cut', el: <TabSwap />},
  {
    len: SEC(4.5), out: 'sweep',
    el: (
      <Solo
        clip={{src: 'cfr/after-academy.mp4', start: 40}}
        eyebrow="Academy" title={'The coach,\nnot a footnote to it'}
        note="Thirty-five pieces across five tracks, every one open from install — no tier, no unlock, no percentage complete."
        height={900} flip shot={{z: [1.03, 1.0], x: [-18, 12]}}
      />
    ),
  },
  {
    len: SEC(7.5), out: 'dissolve',
    el: (
      <OnboardingBeat
        before={ONBOARDING_0_8_9} after={ONBOARDING_0_9}
        title="Fifteen pages down to nine"
        line="The plan-mode fork leads. Everything optional folds into one last page you can skip outright."
      />
    ),
  },
  {
    len: SEC(5.5), out: 'dissolve',
    el: (
      <ListCard
        eyebrow="Also in 0.9" title="The small things"
        items={[
          'Recovery is now Wearable — all three watch options on one row',
          'Tag a finished workout: deload, test, technique',
          'Filters for every exercise, and for your gear',
          'Create a custom exercise from the logger’s search',
          'Hold Start to skip the warm-up',
          'History groups by day — TODAY, YESTERDAY, SAT · AUG 22',
          'The gallery is ungated — every cell opens it',
          'Ember, a second warm accent beside the default Red',
        ]}
      />
    ),
  },
  {len: SEC(4), el: <Card eyebrow="Avex" title="0.9" sub="out now" shot={{z: [1, 1.05]}} />},
];

/* ── assembly ────────────────────────────────────────────────────────────── */

/**
 * A transition of T frames overlaps the two sequences it joins by T, so the running time is not the
 * sum of the beats — it is that sum less every join. Computing it rather than hard-coding a duration
 * means re-timing a beat cannot silently leave a black tail on the end of the film.
 */
const starts: number[] = [];
let clock = 0;
for (let i = 0; i < BEATS.length; i++) {
  starts.push(clock);
  clock += BEATS[i].len - (i < BEATS.length - 1 ? overlap(BEATS[i].out ?? 'fade') : 0);
}

export const TOTAL = clock;

const ACTS: Act[] = BEATS.flatMap((b, i) => (b.act ? [{at: starts[i], label: b.act}] : []));

export const Release: React.FC = () => (
  <AbsoluteFill style={{backgroundColor: BG}}>
    <TransitionSeries>
      {BEATS.flatMap((b, i) => {
        const seq = (
          <TransitionSeries.Sequence key={`s${i}`} durationInFrames={b.len}>
            {b.el}
          </TransitionSeries.Sequence>
        );
        if (i === BEATS.length - 1) return [seq];
        const t = transition(b.out ?? 'fade', `t${i}`);
        return t ? [seq, t] : [seq];
      })}
    </TransitionSeries>
    <Rail acts={ACTS} total={TOTAL} lead={SEC(4.2)} tail={SEC(4.6)} />
  </AbsoluteFill>
);
