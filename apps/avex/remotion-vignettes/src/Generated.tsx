import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {ACCENT, ACCENT_A60, MUTED, ON_BG, OUTLINE_A35} from './theme';

/**
 * "Build me a plan" — answer three questions, get a whole week. The loop is SEAMLESS: frame 0 and
 * the final frame are BOTH the finished week, held still, so the restart never jumps. In between the
 * week clears, the three inputs (gear · goal · days) spring in, and a real 3-day Push/Pull/Legs week
 * redraws itself left-to-right. The card plays this twice then FREEZES on that finished week.
 *
 * 240 frames @ 30fps = 8s. Custom.tsx shares this length + held-first / held-last shape so the two
 * cards start, loop, and freeze together.
 */

const INPUT_PILLS = ['FULL GYM', 'BUILD MUSCLE', '3 DAYS'];

// Three days, three exercises each — reps shaped by the goal (that's the point of the pills). Short
// names so nothing truncates at strip size.
const WEEK: {day: string; ex: [string, string][]}[] = [
  {day: 'PUSH', ex: [['Bench', '3×8'], ['Incline', '3×10'], ['Lateral', '3×12']]},
  {day: 'PULL', ex: [['Pulldown', '3×10'], ['Row', '3×10'], ['Curl', '3×12']]},
  {day: 'LEGS', ex: [['Squat', '4×6'], ['Leg press', '3×10'], ['Calf', '3×15']]},
];

// Phase timeline (frames @ 30fps). Opens HELD on the week, clears it, runs the inputs → rebuild, then
// closes HELD on the week — so the loop seam (last frame → frame 0) is week → week, no jump.
const CLEAR_START = 34; // the opening week starts to clear
const CLEAR_END = 50;
const PILLS_IN = 56; // first input pill enters
const PILL_STAGGER = 7;
const PILLS_OUT = 92; // pills start lifting away
const PILLS_GONE = 104;
const CARDS_IN = 100; // first day frame redraws (overlaps the pills leaving)
const CARD_STAGGER = 8;
const LINES_IN = 124; // first exercise line; sweeps left-to-right across the week
const LINE_STAGGER = 6;

const GAP = 24; // between day columns
const RADIUS = 16;
const PAD = 22;

/** 1 while the opening week is held, 1→0 as it clears, 0 thereafter. */
const heldOut = (frame: number): number =>
  interpolate(frame, [CLEAR_START, CLEAR_END], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});

export const Generated: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, width, height} = useVideoConfig();
  const colW = (width - GAP * (WEEK.length - 1)) / WEEK.length;
  const held = heldOut(frame);

  const pillsLeave = interpolate(frame, [PILLS_OUT, PILLS_GONE], [0, 1], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });

  return (
    <AbsoluteFill>
      {/* Interlude — the three inputs. Hidden while the week is held (before the clear) and once gone. */}
      {pillsLeave < 1 && (
        <AbsoluteFill
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 28,
            opacity: 1 - pillsLeave,
            transform: `translateY(${-30 * pillsLeave}px)`,
          }}
        >
          {INPUT_PILLS.map((label, i) => {
            const enter = spring({
              frame: frame - (PILLS_IN + i * PILL_STAGGER),
              fps,
              config: {damping: 14, stiffness: 130},
            });
            const isGoal = i === 1;
            return (
              <div
                key={label}
                style={{
                  border: `3px solid ${isGoal ? ACCENT_A60 : OUTLINE_A35}`,
                  borderRadius: 999,
                  padding: '16px 34px',
                  fontFamily: 'monospace',
                  fontSize: 38,
                  letterSpacing: 3,
                  color: isGoal ? ON_BG : MUTED,
                  opacity: enter,
                  transform: `scale(${0.82 + 0.18 * enter})`,
                }}
              >
                {label}
              </div>
            );
          })}
        </AbsoluteFill>
      )}

      {/* The week — held at both ends, cleared + rebuilt in the middle. */}
      {WEEK.map((col, c) => {
        // appear: 1 held, 0 cleared, springs back to 1 on rebuild, 1 held again — frame 0 and the
        // last frame both land on 1, so the seam and the freeze are the same finished week.
        const rebuild = spring({frame: frame - (CARDS_IN + c * CARD_STAGGER), fps, config: {damping: 18, stiffness: 130}});
        const appear = Math.min(1, held + rebuild);
        if (appear <= 0) return null;
        const firstLine = LINES_IN + c * col.ex.length * LINE_STAGGER;
        const lastLine = firstLine + (col.ex.length - 1) * LINE_STAGGER;
        // The frame warms to the accent only while this day is being (re)written, calm at both holds.
        const writing = interpolate(
          frame,
          [firstLine - 6, firstLine + 4, lastLine + 10, lastLine + 24],
          [0, 1, 1, 0],
          {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'}
        );
        return (
          <div
            key={col.day}
            style={{
              position: 'absolute',
              left: c * (colW + GAP),
              top: 0,
              width: colW,
              height,
              borderRadius: RADIUS,
              border: `4px solid ${writing > 0.5 ? ACCENT_A60 : OUTLINE_A35}`,
              padding: PAD,
              boxSizing: 'border-box',
              display: 'flex',
              flexDirection: 'column',
              opacity: appear,
              transform: `translateY(${(1 - appear) * 22}px)`,
            }}
          >
            <div style={{display: 'flex', alignItems: 'center', gap: 12, marginBottom: 14}}>
              <span
                style={{
                  fontFamily: 'monospace',
                  fontSize: 27,
                  letterSpacing: 5,
                  color: ACCENT,
                  opacity: 0.55 + 0.45 * appear,
                }}
              >
                {col.day}
              </span>
              <div style={{flex: 1, height: 2, background: ACCENT, opacity: 0.25 + 0.4 * writing}} />
            </div>
            {col.ex.map(([name, sets], i) => {
              const rowRebuild = spring({frame: frame - (firstLine + i * LINE_STAGGER), fps, config: {damping: 16, stiffness: 150}});
              const rowAppear = Math.min(1, held + rowRebuild);
              return (
                <div
                  key={name}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'baseline',
                    flex: 1,
                    opacity: rowAppear,
                    transform: `translateX(${(1 - rowAppear) * 18}px)`,
                  }}
                >
                  <span
                    style={{fontFamily: 'sans-serif', fontSize: 34, color: ON_BG, whiteSpace: 'nowrap', overflow: 'hidden'}}
                  >
                    {name}
                  </span>
                  <span style={{fontFamily: 'monospace', fontSize: 27, color: MUTED, marginLeft: 12, flexShrink: 0}}>
                    {sets}
                  </span>
                </div>
              );
            })}
          </div>
        );
      })}
    </AbsoluteFill>
  );
};
