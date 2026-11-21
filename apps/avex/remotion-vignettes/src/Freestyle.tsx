import React from 'react';
import {AbsoluteFill, useCurrentFrame} from 'remotion';
import {
  ACCENT,
  BG,
  BUILD_END,
  BUILD_IN,
  CELL,
  CELL_RADIUS,
  DP,
  heldOut,
  HOLD_END,
  LABEL_LINE,
  LABEL_SIZE,
  LABEL_TRACKING,
  MONO,
  MUTED,
  ON_BG,
  OUTLINE,
  RUNG,
  SANS,
  settle,
  smoothstep,
  tracked,
  withAlpha,
} from './theme';

/**
 * "Go with the flow" — no plan, you just log what you did when you did it.
 *
 * A week of time running up to a live NOW, and a checked cell that pops onto it every time
 * something gets logged. What makes it freestyle is everything the other two cards have and this one
 * doesn't: no slots, no set order, no plan. The entries are a mix (a squat day, a run, pull-ups), and
 * they fall where they fell: two days running, then a gap, one, a gap, two more.
 *
 * The axis is real time. Each weekday is one step along it, so the gaps you see ARE the days nothing
 * was logged, the labels read Monday to Sunday in order, and NOW, which walks it at an even pace and
 * stops on each log, ends on the next Monday. (It used to jump MON, THU, FRI, SUN, WED at uneven
 * spacing, meant as "over weeks", and read as days out of order.)
 *
 * It is drawn in marks the app already uses:
 * - A logged entry is Home's trained day (`WeekStrip`): an onBg cell with a check in bg ink, in the
 *   strip's own rounded-square proportion. Circles on a line would be the progress-dot rail that
 *   strip was redesigned away from.
 * - NOW is Home's today: an accent RING, never a fill. It is the only accent on this card, which is
 *   what makes it read as "you are here" (§5), and its label is onBg where the days are muted, as
 *   today's letter is.
 * - The line is Coach's ledger spine, a time axis drawn as data: outline at 0.35, 1dp, broken
 *   around each entry so the entries sit on it rather than over it.
 *
 * Frame 0 and the last frame are the same full timeline, so the seam and the freeze land on it.
 */

// The day axis, in dp on the 282×72 strip. Seven days from MON, then NOW on the next Monday.
const DAY0_X = 22 * DP;
const DAY_PITCH = 35 * DP;
const START_X = 4 * DP;
const END_X = DAY0_X + 7 * DAY_PITCH;

// Timeline, in frames. NOW sets off on BUILD_IN and reaches the present on BUILD_END, at one pace,
// so each entry is logged on the frame NOW crosses its day.
const atX = (x: number): number => BUILD_IN + ((x - START_X) / (END_X - START_X)) * (BUILD_END - BUILD_IN);
const REWIND: [number, number] = [HOLD_END + 2, BUILD_IN]; // it runs back to the start as the log clears

const ENTRIES = [
  {name: 'Squat', day: 'MON', d: 0},
  {name: 'Run', day: 'TUE', d: 1},
  {name: 'Pull-ups', day: 'THU', d: 3},
  {name: 'Bench', day: 'SAT', d: 5},
  {name: 'Row', day: 'SUN', d: 6},
].map((e) => {
  const x = DAY0_X + e.d * DAY_PITCH;
  return {...e, x, at: atX(x)};
});

// Geometry, in dp: name over cell over day, the stack centred on the line.
const NAME_SIZE = 10 * DP;
const NAME_TOP = 13 * DP;
const NAME_LINE = 12 * DP;
const LINE_Y = 37 * DP;
const NODE = 16 * DP;
const NODE_RADIUS = (NODE * CELL_RADIUS) / CELL; // Home's cell corner, kept in proportion
const RING_W = 2 * DP; // Home's today ring
const DAY_TOP = 49 * DP;
const LINE_W = 1 * DP;
const LINE_GAP = 3 * DP; // the spine's break either side of an entry, as Coach's is
/** A label's half-width, so NOW's never runs off either edge of the strip. */
const LABEL_HALF = 11 * DP;
const POP_LEN = 14;

/** Where NOW is: held at the end, rewinding to the start, then stepping through each logged moment. */
const headX = (frame: number): number => {
  if (frame < BUILD_IN) return END_X + (START_X - END_X) * smoothstep(frame, REWIND[0], REWIND[1]);
  const keys = [[BUILD_IN, START_X], ...ENTRIES.map((e) => [e.at, e.x]), [BUILD_END, END_X]];
  for (let k = 1; k < keys.length; k++) {
    if (frame <= keys[k][0]) {
      const [f0, x0] = keys[k - 1];
      const [f1, x1] = keys[k];
      return x0 + (x1 - x0) * smoothstep(frame, f0, f1);
    }
  }
  return END_X;
};

/** How far entry `e` has popped in: 1 while held, then a settle from the frame it is logged. */
const popOf = (e: (typeof ENTRIES)[number], frame: number, held: number): number =>
  held > 0 ? 1 : settle(frame, e.at, POP_LEN);

/** The spine's visible runs: START_X up to NOW's ring, less a gap around every entry on it. */
const spine = (frame: number, held: number, head: number): Array<[number, number]> => {
  const end = head - NODE / 2 - LINE_GAP;
  const holes = ENTRIES.map((e) => {
    const r = (NODE / 2 + LINE_GAP) * Math.min(1, Math.max(0, popOf(e, frame, held)));
    return [e.x - r, e.x + r] as [number, number];
  }).filter(([a, b]) => b > a);
  const runs: Array<[number, number]> = [];
  let from = START_X;
  for (const [a, b] of holes) {
    if (a > from && from < end) runs.push([from, Math.min(a, end)]);
    from = Math.max(from, b);
  }
  if (end > from) runs.push([from, end]);
  return runs;
};

/** The check inside a node, on the node's own -8..8 box. */
const CHECK = 'M -3.6 0.2 L -1.1 2.7 L 3.8 -2.4';

export const Freestyle: React.FC = () => {
  const frame = useCurrentFrame();
  const held = heldOut(frame);
  const head = headX(frame);
  // NOW's own label steps aside while NOW sits on an entry, whose day label is in the same place.
  const clear = ENTRIES.reduce((m, e) => {
    const shown = held > 0 || frame >= e.at;
    return shown ? Math.min(m, smoothstep(Math.abs(head - e.x), 20 * DP, 28 * DP)) : m;
  }, 1);

  return (
    <AbsoluteFill>
      {/* The spine, drawn up to NOW. */}
      {spine(frame, held, head).map(([a, b], i) => (
        <div
          key={i}
          style={{
            position: 'absolute',
            left: a,
            top: LINE_Y - LINE_W / 2,
            width: b - a,
            height: LINE_W,
            background: withAlpha(OUTLINE, RUNG.rule),
          }}
        />
      ))}

      {ENTRIES.map((e) => {
        const pop = popOf(e, frame, held);
        const alpha = held > 0 ? held : Math.min(1, pop * 1.5);
        if (alpha <= 0) return null;
        return (
          <React.Fragment key={e.name}>
            <svg
              width={NODE}
              height={NODE}
              viewBox="-8 -8 16 16"
              style={{
                position: 'absolute',
                left: e.x - NODE / 2,
                top: LINE_Y - NODE / 2,
                opacity: alpha,
                transform: `scale(${pop})`,
              }}
            >
              <rect x={-8} y={-8} width={16} height={16} rx={(8 * NODE_RADIUS) / (NODE / 2)} fill={ON_BG} />
              <path d={CHECK} fill="none" stroke={BG} strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            <Label text={e.name} x={e.x} top={NAME_TOP} line={NAME_LINE} size={NAME_SIZE} sans color={ON_BG} alpha={alpha} />
            <Label text={e.day} x={e.x} top={DAY_TOP} line={LABEL_LINE} size={LABEL_SIZE} color={MUTED} alpha={alpha} />
          </React.Fragment>
        );
      })}

      {/* NOW: Home's today ring, the live end of the log and the one mark that never clears. */}
      <div
        style={{
          position: 'absolute',
          left: head - NODE / 2,
          top: LINE_Y - NODE / 2,
          width: NODE,
          height: NODE,
          boxSizing: 'border-box',
          borderRadius: NODE_RADIUS,
          border: `${RING_W}px solid ${ACCENT}`,
        }}
      />
      <Label
        text="NOW"
        x={Math.min(1128 - LABEL_HALF, Math.max(LABEL_HALF, head))}
        top={DAY_TOP}
        line={LABEL_LINE}
        size={LABEL_SIZE}
        color={ON_BG}
        alpha={clear}
      />
    </AbsoluteFill>
  );
};

/** One line of type centred on `x`: sans for a name, else a tracked mono label. */
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
      ...(sans ? {} : tracked(LABEL_TRACKING)),
      color,
      opacity: alpha,
      whiteSpace: 'nowrap',
    }}
  >
    {text}
  </div>
);
