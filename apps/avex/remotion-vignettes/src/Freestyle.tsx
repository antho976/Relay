import React from 'react';
import {AbsoluteFill, useCurrentFrame} from 'remotion';
import {ACCENT, bump, DP, heldOut, HOLD_END, MONO, MUTED, ON_BG, SANS, settle, smoothstep, withAlpha} from './theme';

/**
 * "Go with the flow" — no plan, you just log what you did when you did it.
 *
 * A timeline running up to a live NOW point, and a check that pops onto it every time something gets
 * logged. Checks are the at-a-glance word for "done, recorded". What makes it freestyle is everything
 * the other two cards have and this one doesn't: no Monday-to-Sunday row, no slots, no set order. The
 * entries are a mix (a squat day, a run, pull-ups), the days they land on skip around, and the gaps
 * between them are uneven because NOW itself moves at an uneven pace, pausing and hurrying the way a
 * real month does.
 *
 * Frame 0 and the last frame are the same full timeline, so the seam and the freeze land on it.
 */

const ENTRIES = [
  {name: 'Squat', day: 'MON', x: 22 * DP, at: 50},
  {name: 'Run', day: 'THU', x: 72 * DP, at: 60},
  {name: 'Pull-ups', day: 'FRI', x: 122 * DP, at: 66},
  {name: 'Bench', day: 'SUN', x: 186 * DP, at: 94},
  {name: 'Row', day: 'WED', x: 234 * DP, at: 106},
];

// Geometry, in dp on the 282×72 strip: name over node over day, the stack centred.
const NAME_TOP = 13 * DP;
const NAME_LINE = 12 * DP;
const NAME_SIZE = 10 * DP;
const LINE_Y = 37 * DP;
const NODE_R = 8 * DP;
const DAY_TOP = 49 * DP;
const DAY_SIZE = 9 * DP;
const LINE_W = 0.75 * DP;
const HEAD_R = 3.5 * DP;
const START_X = 4 * DP;
const END_X = 268 * DP;

// Timeline, in frames.
const BUILD_IN = 38; // NOW sets off from the start…
const BUILD_DONE = 118; // …and reaches the present here
const REWIND: [number, number] = [HOLD_END + 2, BUILD_IN]; // it runs back to the start as the log clears

/** Where NOW is: held at the end, rewinding to the start, then stepping through each logged moment. */
const headX = (frame: number): number => {
  if (frame < BUILD_IN) return END_X + (START_X - END_X) * smoothstep(frame, REWIND[0], REWIND[1]);
  const keys = [[BUILD_IN, START_X], ...ENTRIES.map((e) => [e.at, e.x]), [BUILD_DONE, END_X]];
  for (let k = 1; k < keys.length; k++) {
    if (frame <= keys[k][0]) {
      const [f0, x0] = keys[k - 1];
      const [f1, x1] = keys[k];
      return x0 + (x1 - x0) * smoothstep(frame, f0, f1);
    }
  }
  return END_X;
};

/** The check inside a node, on the node's own -8..8 box. */
const CHECK = 'M -3.6 0.2 L -1.1 2.7 L 3.8 -2.4';

export const Freestyle: React.FC = () => {
  const frame = useCurrentFrame();
  const held = heldOut(frame);
  const head = headX(frame);
  const nudge = ENTRIES.reduce((m, e) => Math.max(m, bump(frame, e.at - 4, 10)), 0);
  // NOW's own label steps aside while NOW sits on a check, whose day label is in the same place.
  const clear = ENTRIES.reduce((m, e) => {
    const shown = held > 0 || frame >= e.at;
    return shown ? Math.min(m, smoothstep(Math.abs(head - e.x), 16 * DP, 28 * DP)) : m;
  }, 1);

  return (
    <AbsoluteFill>
      {/* The timeline, drawn up to NOW. */}
      <div
        style={{
          position: 'absolute',
          left: START_X,
          top: LINE_Y - LINE_W / 2,
          width: Math.max(0, head - START_X),
          height: LINE_W,
          borderRadius: LINE_W / 2,
          background: withAlpha(MUTED, 0.35),
        }}
      />

      {ENTRIES.map((e) => {
        const pop = held > 0 ? 1 : settle(frame, e.at, 14);
        const alpha = held > 0 ? held : Math.min(1, pop * 1.5);
        if (alpha <= 0) return null;
        const ring = held > 0 ? 0 : smoothstep(frame, e.at, e.at + 16);
        const ringR = NODE_R * (1 + ring);
        return (
          <React.Fragment key={e.name}>
            {ring > 0 && ring < 1 && (
              <div
                style={{
                  position: 'absolute',
                  left: e.x - ringR,
                  top: LINE_Y - ringR,
                  width: ringR * 2,
                  height: ringR * 2,
                  boxSizing: 'border-box',
                  borderRadius: '50%',
                  border: `${LINE_W}px solid ${withAlpha(ACCENT, 0.6 * (1 - ring))}`,
                }}
              />
            )}
            <svg
              width={NODE_R * 2}
              height={NODE_R * 2}
              viewBox="-8 -8 16 16"
              style={{
                position: 'absolute',
                left: e.x - NODE_R,
                top: LINE_Y - NODE_R,
                opacity: alpha,
                transform: `scale(${pop})`,
              }}
            >
              <circle r={8} fill={ACCENT} />
              <path d={CHECK} fill="none" stroke={ON_BG} strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            <Label text={e.name} x={e.x} top={NAME_TOP} line={NAME_LINE} size={NAME_SIZE} sans color={ON_BG} alpha={alpha} />
            <Label text={e.day} x={e.x} top={DAY_TOP} line={DAY_SIZE} size={DAY_SIZE} color={MUTED} alpha={alpha} />
          </React.Fragment>
        );
      })}

      {/* NOW: the live end of the log, and the one mark that never clears. It nudges as each check lands. */}
      <div
        style={{
          position: 'absolute',
          left: head - HEAD_R,
          top: LINE_Y - HEAD_R,
          width: HEAD_R * 2,
          height: HEAD_R * 2,
          borderRadius: '50%',
          background: ACCENT,
          transform: `scale(${1 + 0.35 * nudge})`,
        }}
      />
      <Label text="NOW" x={head} top={DAY_TOP} line={DAY_SIZE} size={DAY_SIZE} color={MUTED} alpha={clear} />
    </AbsoluteFill>
  );
};

/** One line of type centred on `x`. */
const Label: React.FC<{
  text: string;
  x: number;
  top: number;
  line: number;
  size: number;
  sans?: boolean;
  color: string;
  alpha: number;
}> = ({text, x, top, line, size, sans, color, alpha}) => (
  <div
    style={{
      position: 'absolute',
      left: x - 40 * DP,
      width: 80 * DP,
      top,
      textAlign: 'center',
      fontFamily: sans ? SANS : MONO,
      fontWeight: sans ? 500 : 400,
      fontSize: size,
      lineHeight: `${line}px`,
      color,
      opacity: alpha,
      whiteSpace: 'nowrap',
    }}
  >
    {text}
  </div>
);
