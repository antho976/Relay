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
import {Edges} from './Type';
import {Xit, overlap, transition} from './Transitions';
import {BG, bar, snap} from './theme';

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
 * The deliberate syncs: Home turns red on bar 5 as the bed enters, the chart lands on bar 25 as it
 * drops back, and the fifth tab changes hands on bar 33, the frame the bed comes back at full
 * strength after cutting away for a bar.
 */

type Beat = {
  /** the bar this beat hands over on */
  endBar: number;
  out?: Xit;
  shot?: Shot;
  /** `start` is the beat's own first frame, so a beat can place its own events on the grid */
  el: (start: number) => React.ReactNode;
};

/**
 * Bar allocation. The bed's own shape, measured per bar: near-silent 1-4, enters at 5, builds to
 * 16, opens up at 17, drops back at 25, builds again to 31, cuts out for bar 32, returns at 33 and
 * runs full until it drops at 45. The cuts that matter sit on those bars: Home turns red on 5, the
 * watch hands to Coach on 17, the chart lands on 25, the fifth tab changes hands on 33, and the
 * closing line lands on 45.
 *
 * Against the previous cut: Coach was six bars, of which four and a half seconds were a still
 * screen waiting for the capture to start moving — it is five, with the tour starting a bar after
 * the seam. That bar and one from the watch (four to three) went to the notifications beat, which
 * carries a paragraph and needed six seconds to be read, and to Profile, whose capture now has
 * time to scroll to the month grid the copy is about.
 */
const BEATS: Beat[] = [
  /* ── open ─────────────────────── two bars; the bed is near-silent until bar 5 ── */
  {
    endBar: 3, out: 'dissolve', shot: {z: [1.06, 1], ease: EASE.glide},
    el: () => <Card eyebrow="What's new" title="Avex 0.9" count={[147, 632]} />,
  },

  /* ── the day ────────────────────────────────────────────── bed enters, bar 5 ── */
  {
    endBar: 6, out: 'whip', shot: {z: [1, 1.05], x: [0, -20]},
    /* The era turns over on bar 5, the frame the bed enters: the accent going red and the music
       arriving are one event. */
    el: (start) => <HomeMorph gridStart={start} turnAt={bar(5) - start} />,
  },
  {endBar: 9, out: 'push', shot: {z: [1.04, 1], x: [24, 0]}, el: (start) => <NoticeBeat gridStart={start} />},

  /* ── the session ─────────────────────────────────── bed opens up at bar 17 ── */
  {
    endBar: 12, out: 'cut', shot: {z: [1, 1.05], x: [0, -16], ease: EASE.drift},
    el: (start) => (
      <Compare
        before={{src: 'cfr/before-session.mp4', start: 690}}
        after={{src: 'cfr/after-session.mp4', start: 890}}
        eyebrow="Your workout" title={'The screen you\ntrain on'}
        note="The exercise name used to take up half the screen. Now your last session and what to lift next show up together, the rest timer runs on its own, and one button finishes an exercise."
        hold={bar(10) - start} sweep={30} dir="ltr" height={880}
      />
    ),
  },
  {
    endBar: 14, out: 'push', shot: {z: [1, 1.045], x: [0, -14], y: [6, -6], ease: EASE.drift},
    el: (start) => {
      // The set is logged on bar 13: the row flips to logged, "Set logged" lands and the rest timer
      // starts running. The capture's own change is at source frame 1083, so the clip starts
      // wherever puts that frame on the downbeat. This beat used to sit on a still screen for four
      // seconds with no camera, and its one cue fired eight frames in, inside the push.
      const logAt = bar(13) - start;
      return (
        <Solo
          clip={{src: 'cfr/after-session.mp4', start: 1083 - logAt}}
          eyebrow="Your workout" title="It tells you what to lift"
          line="The weight to try next, and why. What you beat last time, right there on the row. How many reps you need for a personal record."
          height={900} flip cues={[{at: logAt, sfx: 'confirm'}]}
        />
      );
    },
  },
  {endBar: 17, out: 'whip', shot: {z: [1.03, 1], y: [-10, 0], ease: EASE.drift}, el: (start) => <WatchBeat gridStart={start} />},

  /* ── the record ────────────────────────────────── bed drops back at bar 25 ── */
  {
    /* Five bars, one continuous shot. The seam happens at the top, where 0.8.9's volume figure
       reads "52...." against 0.9's "52.8k lb"; a bar after the edge has passed, the capture tours
       down to the block rail on its own and holds there. The after clip is profiled still from
       source 424 to 633 and moving from 633; at 0.8x, starting from 513, the tour begins on bar
       19.5 and the beat ends on the block before the capture starts scrolling back up at 766. */
    endBar: 22, out: 'cut', shot: {z: [1, 1.05], x: [0, 18], ease: EASE.drift},
    el: (start) => (
      <Compare
        before={{src: 'cfr/before-coach.mp4', start: 180}}
        after={{src: 'cfr/after-coach.mp4', start: 513, rate: 0.8}}
        eyebrow="Coach" title="Coach shows its work"
        note="Your week, your recovery, and where you are in a training block, all on one page. The volume figure stops getting cut off halfway through. And the block itself is new: build up, push hard, peak, then take a lighter week, started straight from the brief."
        hold={bar(18) - start} sweep={30} dir="ltr" height={880} flip
      />
    ),
  },
  {
    endBar: 25, out: 'cut', shot: {z: [1.04, 1], ease: EASE.drift},
    el: (start) => {
      // Two views: the seam reveals 0.9's week, then on beat 4 of bar 23 the toggle goes to
      // Progress and the pace trend takes over. The toggle is at source frame 162 of the capture.
      const toggleAt = snap(bar(23) + 45, 4) - start;
      return (
        <Compare
          before={{src: 'cfr/before-cardio.mp4', start: 0}}
          after={{src: 'cfr/after-cardio.mp4', start: 162 - toggleAt}}
          eyebrow="Cardio" title="Built around your week"
          line="Two views instead of one long list. This week at a glance, or how you are trending over time."
          hold={bar(23) - start} sweep={24} dir="ttb" height={880}
          cues={[{at: toggleAt, sfx: 'reveal', gain: 0.5}]}
        />
      );
    },
  },
  {
    /* Cut in on bar 25, the bar the bed drops back, with the chart already standing: the picture
       and the music change on the same frame. */
    endBar: 27, out: 'pushUp', shot: {z: [1.05, 1], y: [10, -6], ease: EASE.drift},
    el: () => (
      <Solo
        clip={{src: 'cfr/after-cardio.mp4', start: 822}}
        eyebrow="New in 0.9" title="Every week as a chart"
        line="One bar per week, back to your very first, against the 150 minutes a week the WHO recommends."
        height={900}
      />
    ),
  },
  {
    endBar: 29, out: 'push', shot: {z: [1, 1.04], y: [8, -8]},
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
    /* before-lasttab is only still for its first 89 frames, so the seam sits on beat 3 of the
       first bar rather than on a downbeat: the earliest grid point that lets it complete while both
       captures are still. It used to be at a bare frame 40, off the grid. Three bars, so the after
       capture has time to scroll down to the month grid the copy is about (source 131 onward). */
    endBar: 32, out: 'dissolve', shot: {z: [1, 1.04], x: [0, 12], ease: EASE.drift},
    el: (start) => (
      <Compare
        before={{src: 'cfr/before-lasttab.mp4', start: 4}}
        after={{src: 'cfr/after-profile.mp4', start: 6}}
        eyebrow="Profile" title="Easier to read"
        line="Your year used to be twelve rows of dots. Now it is a month you can actually read, and the photo gallery is open to everyone."
        hold={bar(29) + 30 - start} sweep={26} dir="ltr" height={880} flip
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
        height={900} flip
      />
    ),
  },
  {
    endBar: 41, out: 'dissolve',
    el: (start) => (
      <OnboardingBeat
        before={ONBOARDING_0_8_9} after={ONBOARDING_0_9}
        gridStart={start}
        /* The hit is the loudest single event in the film, so it goes on a downbeat rather than
           wherever the fifteen pages happen to finish landing. Bar 39 leaves two beats of stillness
           after the last page, which is what makes the hit land. */
        phase={{bam: bar(39) - start}}
        eyebrow="Setting up"
        title="Fifteen questions down to nine"
        line="It used to ask fifteen things before showing you anything. Now it asks what shapes your plan, and the rest waits until the end."
      />
    ),
  },
  {
    endBar: 44, out: 'dissolve',
    el: (start) => (
      <ListCard
        gridStart={start}
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
    el: (start) => <Card eyebrow="Avex" title="0.9" sub="tomorrow" subAt={bar(45) - start} />,
  },
];

/* ── laying the beats on the grid ────────────────────────────────────────── */

/**
 * A transition of T frames overlaps the two sequences it joins by T, so a beat's length is not
 * simply the gap between downbeats. For the *visible* handover to sit on bar `endBar`, the
 * transition has to straddle it: it starts floor(T/2) frames before the downbeat and the beat runs
 * ceil(T/2) frames past it. With a hard cut both are zero and the cut lands exactly on the downbeat.
 *
 * Everything here is an integer. The previous version split T in half as a float, so every beat
 * after a seven-frame whip started on a half frame, and every cue that beat placed by
 * `bar(n) - start` was rounded away from the grid by Remotion.
 */
const T = (b: Beat) => overlap(b.out ?? 'fade');

const LAYOUT = BEATS.map(() => ({len: 0, start: 0}));
for (let i = 0; i < BEATS.length; i++) {
  const prev = i === 0 ? null : BEATS[i - 1];
  LAYOUT[i].start = prev ? bar(prev.endBar) - Math.floor(T(prev) / 2) : 0;
  const tail = i === BEATS.length - 1 ? 0 : Math.ceil(T(BEATS[i]) / 2);
  LAYOUT[i].len = bar(BEATS[i].endBar) + tail - LAYOUT[i].start;
}

export const TOTAL = LAYOUT[LAYOUT.length - 1].start + LAYOUT[LAYOUT.length - 1].len;

/** Where each handover actually lands, for the render log and the tests in tools/. */
export const HANDOVERS = BEATS.slice(0, -1).map((b, i) => ({
  bar: b.endBar, at: bar(b.endBar), transition: b.out ?? 'fade',
  start: LAYOUT[i + 1].start, mid: LAYOUT[i + 1].start + T(b) / 2,
}));

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
          <TransitionSeries.Sequence key={`s${i}`} durationInFrames={len}>
            {/* No beat fades at a join — the transition is the join. Only the film fades. */}
            <Edges.Provider value={{head: i === 0 ? 12 : 0, tail: i === BEATS.length - 1 ? 16 : 0}}>
              <Camera shot={beat.shot}>{beat.el(start)}</Camera>
            </Edges.Provider>
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
