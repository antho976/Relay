import React from 'react';
import {AbsoluteFill, useCurrentFrame, useVideoConfig} from 'remotion';
import {LABEL_SIZE, MonoLabel, SET_H, SetRow, setsWidth} from './Marks';
import {heldOut, smoothstep} from './theme';

/**
 * "Build me a plan" — your week, named and dated, handed to you.
 *
 * Three rows, and the text is what does the work: MON · WED · FRI says WEEK, and PUSH · PULL · LEGS
 * says what each day is for. They are ALIGNED into a table, because being ordered is the whole
 * argument for the option — set the same rows loose and you have the freestyle card.
 *
 * Two beats, and the pair of them is what makes this read as generated rather than assembled. The
 * three day rows snap in almost together (the plan is decided in one go, not deliberated), then the
 * blocks TALLY out left to right straight across the week without pausing at the row breaks, like a
 * count being run. Custom.tsx lands one row at a time instead, and that difference in rhythm is what
 * separates the two cards.
 *
 * The loop is seamless: frame 0 and the final frame are both the finished week, held still. The card
 * plays it twice then FREEZES on that week — the answer to the question, not the machinery.
 */

/** A real 3-day Push/Pull/Legs week. The blocks are that day's exercises. */
const WEEK = [
  {day: 'MON', split: 'PUSH', work: 5},
  {day: 'WED', split: 'PULL', work: 4},
  {day: 'FRI', split: 'LEGS', work: 5},
];

const ROWS_IN = 40; // the three day rows arrive…
const ROW_STAGGER = 10; // …almost on top of each other
const TALLY_IN = 66; // then the exercises count out across the whole week
const TALLY_STEP = 3.4;

const ROW_PITCH = 84;
const COL_SPLIT = 170; // x of the split name, relative to the row
const COL_WORK = 380; // x of the first exercise block

export const Generated: React.FC = () => {
  const frame = useCurrentFrame();
  const {width, height} = useVideoConfig();
  const held = heldOut(frame);

  const blockW = COL_WORK + setsWidth(Math.max(...WEEK.map((r) => r.work)));
  const left = (width - blockW) / 2;
  const top = (height - (LABEL_SIZE + (WEEK.length - 1) * ROW_PITCH)) / 2;

  // Running index across the WHOLE week, so the tally never resets at a row break.
  let tallied = 0;

  return (
    <AbsoluteFill>
      {WEEK.map((row, r) => {
        const rowAppear = Math.min(1, held + smoothstep(frame, ROWS_IN + r * ROW_STAGGER, ROWS_IN + r * ROW_STAGGER + 12));
        const first = tallied;
        tallied += row.work;
        if (rowAppear <= 0) return null;
        const y = top + r * ROW_PITCH;
        return (
          <React.Fragment key={row.day}>
            <div
              style={{
                position: 'absolute',
                left,
                top: y,
                display: 'flex',
                opacity: rowAppear,
                transform: `translateY(${(1 - rowAppear) * 10}px)`,
              }}
            >
              <MonoLabel opacity={1}>{row.day}</MonoLabel>
              <div style={{position: 'absolute', left: COL_SPLIT}}>
                <MonoLabel opacity={1}>{row.split}</MonoLabel>
              </div>
            </div>
            <div style={{position: 'absolute', left: left + COL_WORK, top: y + (LABEL_SIZE - SET_H) / 2}}>
              <SetRow
                sets={row.work}
                alphaAt={(i) => Math.min(1, held + smoothstep(frame, TALLY_IN + (first + i) * TALLY_STEP, TALLY_IN + (first + i) * TALLY_STEP + 7))}
              />
            </div>
          </React.Fragment>
        );
      })}
    </AbsoluteFill>
  );
};
