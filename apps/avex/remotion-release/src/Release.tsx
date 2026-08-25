import React from 'react';
import {AbsoluteFill} from 'remotion';
import {TransitionSeries, linearTiming, springTiming} from '@remotion/transitions';
import {fade} from '@remotion/transitions/fade';
import {slide} from '@remotion/transitions/slide';
import {wipe} from '@remotion/transitions/wipe';
import {Compare, CompareStills, ListCard, Solo, Spotlight, TitleCard} from './Scenes';
import {GoalCloseUp, HomeMorph, TabSwap, VolumeCount, WatchBeat} from './NativeBeats';
import {BG} from './theme';

const s = (sec: number) => Math.round(sec * 30);
const onb = (dir: string, files: string[]) => files.map((f) => `stills/${dir}/${f}`);

/**
 * Every clip is served from `public/cfr/` — screenrecord writes VARIABLE frame rate (the session
 * takes averaged ~56fps), so a frame index measured with ffmpeg meant a different instant to
 * Remotion, which counts composition frames at 30. The captures are transcoded to constant 30fps so
 * the two agree; only then do the offsets below land where they were measured.
 *
 * Where a pair is retimed, BOTH sides get the SAME span: each traverses that many source frames
 * across the beat, so they stay in step without one running in slow motion against the other.
 */
type Beat = {len: number; el: React.ReactNode; t?: 'fade' | 'slideL' | 'wipe'};

export const BEATS: Beat[] = [
  {len: s(4), el: <TitleCard eyebrow="Release" title="Avex 0.9" sub="145 commits · 460 files" />, t: 'fade'},

  /* ── the day ─────────────────────────────────────────────────────────── */
  {len: s(13), t: 'slideL', el: <HomeMorph />},
  {
    len: s(12), t: 'fade',
    el: (
      <Spotlight
        still="stills/spot/home.png" eyebrow="Home · in detail" title="Six things that moved"
        rings={[
          {x: 9, y: 7, label: 'The bell, with its unread count'},
          {x: 84, y: 7, label: 'Profile and Settings, out of the hop'},
          {x: 38, y: 21, label: 'Days since your last session'},
          {x: 41, y: 25, label: "Today's lifts, at their weights"},
          {x: 84, y: 32, label: 'Plan, split away from Start'},
          {x: 11, y: 63, label: 'Every goal wears its implement'},
        ]}
      />
    ),
  },
  {len: s(11), t: 'wipe', el: <GoalCloseUp />},

  /* ── the session ─────────────────────────────────────────────────────── */
  {
    len: s(10), t: 'slideL',
    el: (
      <Solo
        src="cfr/after-session.mp4" start={420}
        eyebrow="Warm-up" title="Ready before the first rep"
        note="Three mobility drills, then straight into lifting."
      />
    ),
  },
  {
    len: s(15), t: 'slideL',
    el: (
      <Compare
        beforeSrc="cfr/before-session.mp4" afterSrc="cfr/after-session.mp4"
        beforeStart={690} afterStart={890}
        eyebrow="The session" title="The screen you actually train on"
        note="The title stops eating the fold. Last session and suggested next arrive in the same breath, the rest timer keeps its own time, and one button closes out an exercise."
      />
    ),
  },
  {
    len: s(12), t: 'fade',
    el: (
      <Spotlight
        still="stills/spot/session.png" eyebrow="The session · in detail" title="Where the reps get easier"
        rings={[
          {x: 17, y: 24, label: 'Suggested next — and why'},
          {x: 82, y: 42, label: 'What you beat, on the row'},
          {x: 26, y: 46, label: 'Reps left for the PR'},
          {x: 64, y: 65, label: 'Swap without losing the set'},
          {x: 20, y: 74, label: 'Up next, always visible'},
        ]}
      />
    ),
  },
  {len: s(14), t: 'wipe', el: <WatchBeat />},

  /* ── the record ──────────────────────────────────────────────────────── */
  {
    len: s(13), t: 'slideL',
    el: (
      <Compare
        beforeSrc="cfr/before-coach.mp4" afterSrc="cfr/after-coach.mp4"
        beforeStart={107} afterStart={179} beforeSpan={460} afterSpan={460}
        eyebrow="Coach" title="It shows its working"
        note="Signals, recovery load and the week's block in one place — and a volume figure that no longer truncates to 47…"
      />
    ),
  },
  {
    len: s(11), t: 'wipe',
    el: (
      <Solo
        src="cfr/after-coach.mp4" start={520}
        eyebrow="Coach · lifts on watch" title="Every lift, trending"
        note="A sparkline and a percentage beside each movement, so a stall shows up before you feel it."
      />
    ),
  },
  {
    len: s(13), t: 'slideL',
    el: (
      <Compare
        beforeSrc="cfr/before-cardio.mp4" afterSrc="cfr/after-cardio.mp4"
        beforeStart={31} afterStart={263} beforeSpan={340} afterSpan={340}
        eyebrow="Cardio" title="One week, two lenses"
        note="Rebuilt around the week in front of you, with a WEEK and a PROGRESS lens instead of one long list."
      />
    ),
  },
  {
    len: s(11), t: 'wipe',
    el: (
      <Solo
        src="cfr/after-cardio.mp4" start={950}
        eyebrow="Cardio · new" title="Weeks, drawn — not listed"
        note="Tap the strip and the whole history opens as a bar chart against your weekly target."
      />
    ),
  },
  {
    len: s(10), t: 'fade',
    el: (
      <Solo
        src="cfr/after-cardio.mp4" start={1350}
        eyebrow="History" title="Everything you did, searchable"
        note="One list across lifting and cardio, filterable by length and effort."
      />
    ),
  },
  {
    len: s(13), t: 'slideL',
    el: (
      <Compare
        beforeSrc="cfr/before-lasttab.mp4" afterSrc="cfr/after-profile.mp4"
        beforeStart={30} afterStart={72} beforeSpan={287} afterSpan={287}
        eyebrow="Profile" title="A year that fits on the page"
        note="The twelve-row year grid becomes a month you can actually read, tallies pair off, and the gallery is open to everyone."
      />
    ),
  },
  {len: s(10), t: 'wipe', el: <VolumeCount />},

  /* ── the shape of it ─────────────────────────────────────────────────── */
  {len: s(11), t: 'fade', el: <TabSwap />},
  {
    len: s(12), t: 'wipe',
    el: (
      <Solo
        src="cfr/after-academy.mp4" start={120}
        eyebrow="Academy" title="The coach, not a footnote to it"
        note="Twenty-eight lessons rewritten in plain language, with a For You shelf that keeps moving."
      />
    ),
  },
  {
    len: s(14), t: 'slideL',
    el: (
      <CompareStills
        before={onb('before-onboarding', ['01.png','02.png','03.png','04.png','05.png','06.png','07.png','08.png','09.png','10.png','11.png','12.png','13.png','14.png','15.png','16.png','17.png'])}
        after={onb('after-onboarding', ['01.png','02.png','03.png','04.png','05.png','06.png','07.png','08.png','09.png','10-landed.png'])}
        eyebrow="Onboarding" title="Seventeen pages down to ten"
        note="Plan first, then only the questions that shape it — and everything optional folded into one last page you can skip outright."
      />
    ),
  },
  {
    len: s(13), t: 'fade',
    el: (
      <ListCard
        eyebrow="Also in 0.9" title="The small things"
        items={[
          'Recovery is now Wearable — all three watch options on one row',
          'Notifications split by phone and in-app, with quiet hours',
          'Filters for every exercise and for your gear',
          'Create a custom exercise straight from the logger search',
          'One button to finish an exercise, not a hunt for the next',
          'Recent and History rebuilt around what you actually did',
          'The gallery is ungated — no tier, no unlock',
          'Phone notifications wear the Avex mark',
        ]}
      />
    ),
  },
  {len: s(5), el: <TitleCard eyebrow="Avex" title="0.9" sub="out now" />},
];

const T = 18;

const transitionFor = (t: Beat['t'], key: string) => {
  if (t === 'slideL') {
    return <TransitionSeries.Transition key={key} presentation={slide({direction: 'from-right'})} timing={linearTiming({durationInFrames: T})} />;
  }
  if (t === 'wipe') {
    return <TransitionSeries.Transition key={key} presentation={wipe({direction: 'from-bottom-right'})} timing={springTiming({config: {damping: 200}, durationInFrames: T + 6})} />;
  }
  return <TransitionSeries.Transition key={key} presentation={fade()} timing={linearTiming({durationInFrames: T})} />;
};

export const Release: React.FC = () => (
  <AbsoluteFill style={{backgroundColor: BG}}>
    <TransitionSeries>
      {BEATS.flatMap((b, i) => {
        const seq = (
          <TransitionSeries.Sequence key={`s${i}`} durationInFrames={b.len}>{b.el}</TransitionSeries.Sequence>
        );
        return i === BEATS.length - 1 ? [seq] : [seq, transitionFor(b.t, `t${i}`)];
      })}
    </TransitionSeries>
  </AbsoluteFill>
);

export const TOTAL = BEATS.reduce((n, b) => n + b.len, 0) - (BEATS.length - 1) * T;
