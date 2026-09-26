import React from 'react';
import {AbsoluteFill, useCurrentFrame} from 'remotion';
import {ACCENT, DP, hash01, heldOut, MONO, MUTED, mix, ON_BG, OUTLINE, settle, smoothstep} from './theme';

/**
 * "Build me a plan" — the app decides your week.
 *
 * It is drawn in the mark this path is about to show you for real: `PlanLedger`, one bar per day
 * carrying that day's sets. So the first thing a new user sees of the generated path is the thing
 * the next pages build, not a diagram of it.
 *
 * The motion is the argument. All seven tracks start SHUFFLING in a muted grey, like a solver trying
 * candidate weeks, then lock left to right: the training days land in accent at their volume and the
 * rest days drop to empty. Resting is part of the decision, so rest days go through the same shuffle
 * and are seen to be chosen. Custom lands one tile per tap and freestyle fills in over weeks; this one
 * is the only card where nothing you do is shown, because on this path you don't do it.
 *
 * Frame 0 and the last frame are the same finished week, held still, so the loop seam and the freeze
 * both land on it.
 */

/** A four-day upper/lower week. The value is that day's sets, 0 = rest. */
const WEEK = [
  {day: 'M', sets: 22},
  {day: 'T', sets: 18},
  {day: 'W', sets: 0},
  {day: 'T', sets: 24},
  {day: 'F', sets: 16},
  {day: 'S', sets: 0},
  {day: 'S', sets: 0},
];
const PEAK = Math.max(...WEEK.map((d) => d.sets));

// Geometry, in dp on the 282×72 strip.
const BAR_W = 16 * DP;
const PITCH = 36 * DP;
const TRACK_TOP = 5 * DP;
const TRACK_H = 45 * DP;
const LABEL_GAP = 7 * DP;
const LABEL_SIZE = 10 * DP; // labelS
const RADIUS = 4 * DP; // PlanLedger's bar corner

// Timeline, in frames.
const SHUFFLE_IN = 36; // column 0 starts trying heights…
const SHUFFLE_STAGGER = 2;
const LOCK_IN = 58; // …and locks here
const LOCK_STAGGER = 7; // one column per ~quarter second, left to right
const LOCK_LEN = 18;
const STEP = 3; // frames each candidate height is held while shuffling

/** Candidate height while a column shuffles: 0.2..1, a new value every [STEP] frames, eased between. */
export const shuffleAt = (col: number, frame: number): number => {
  const k = Math.floor(frame / STEP);
  const a = hash01(col, k);
  const b = hash01(col, k + 1);
  return 0.2 + 0.8 * (a + (b - a) * smoothstep(frame - k * STEP, 0, STEP));
};

export const Generated: React.FC = () => {
  const frame = useCurrentFrame();
  const held = heldOut(frame);
  const left = (1128 - (6 * PITCH + BAR_W)) / 2;

  return (
    <AbsoluteFill>
      {WEEK.map((d, i) => {
        const target = d.sets / PEAK;
        const lockAt = LOCK_IN + i * LOCK_STAGGER;
        const trying = smoothstep(frame, SHUFFLE_IN + i * SHUFFLE_STAGGER, SHUFFLE_IN + i * SHUFFLE_STAGGER + 8);
        const from = shuffleAt(i, lockAt);
        const built = frame < lockAt ? trying * shuffleAt(i, frame) : from + (target - from) * settle(frame, lockAt, LOCK_LEN);
        const height = Math.max(0, held * target + built);
        // How far this day has turned from "being considered" (muted) to "decided" (accent).
        const decided = d.sets > 0 ? Math.max(held, smoothstep(frame, lockAt, lockAt + 6)) : 0;
        const x = left + i * PITCH;
        return (
          <React.Fragment key={i}>
            <div
              style={{
                position: 'absolute',
                left: x,
                top: TRACK_TOP,
                width: BAR_W,
                height: TRACK_H,
                borderRadius: RADIUS,
                background: OUTLINE,
              }}
            />
            {height > 0.001 && (
              <div
                style={{
                  position: 'absolute',
                  left: x,
                  top: TRACK_TOP + TRACK_H * (1 - Math.min(1.06, height)),
                  width: BAR_W,
                  height: TRACK_H * Math.min(1.06, height),
                  borderRadius: RADIUS,
                  background: mix(MUTED, ACCENT, decided),
                  opacity: 0.35 + 0.65 * decided,
                }}
              />
            )}
            <div
              style={{
                position: 'absolute',
                left: x,
                width: BAR_W,
                top: TRACK_TOP + TRACK_H + LABEL_GAP,
                textAlign: 'center',
                fontFamily: MONO,
                fontSize: LABEL_SIZE,
                lineHeight: `${LABEL_SIZE}px`,
                color: mix(MUTED, ON_BG, decided),
              }}
            >
              {d.day}
            </div>
          </React.Fragment>
        );
      })}
    </AbsoluteFill>
  );
};
