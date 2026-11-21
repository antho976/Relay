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
  LABEL_LINE,
  LABEL_SIZE,
  LABEL_TRACKING,
  mix,
  MONO,
  MUTED,
  ON_BG,
  RUNG,
  settle,
  smoothstep,
  tracked,
  withAlpha,
} from './theme';

/**
 * "I'll make my own" — you decide what goes on which day.
 *
 * The week is laid out empty, seven dashed cells, and each training day is DRAGGED into the cell it
 * chose: PUSH onto Monday, PULL onto Wednesday, LEGS onto Friday. Dragging is the one gesture no
 * other card has, and it is the whole difference between this option and the generated one: the
 * same week, but every day in it was put there by hand. The days you leave empty stay dashed, so the
 * frozen frame still reads as a week you could keep filling.
 *
 * Everything is drawn in a mark the app already uses, so nothing here has to be learned twice:
 * - The cell is Home's week-strip cell (`WeekStrip`, `CellShape`): 34dp, 12dp corners. An empty one
 *   is that strip's dashed outline (muted 0.6, 1.5dp, 4/3dp dashes), which there means a day with no
 *   session on it, the same thing it means here.
 * - A tile being carried looks the way a bar being dragged in `WeekBarRail` does: lifted to 1.06 and
 *   in accent, the one thing on the card under your finger. It drops to scale 1 and settles into an
 *   onBg fill, Home's filled day. So accent is spent on one tile at a time, at size, and the finished
 *   week rests without any (§5), and the tile's label never changes colour: bg is both Home's ink on
 *   an onBg cell and the theme's onPrimary for the default Red.
 * There is no finger. The app shows a press as scale and a drag as lift, never as a touch dot, and
 * the dot (a grey blob over the label, off the alpha ladder) was the one thing on the card that read
 * as a glitch.
 *
 * Frame 0 and the last frame are the same three placed days, so the seam and the freeze land on it.
 */

const PLACED = [
  {slot: 0, label: 'PUSH'},
  {slot: 2, label: 'PULL'},
  {slot: 4, label: 'LEGS'},
];
const DAYS = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];

// Geometry, in dp on the 282×72 strip: cell over day letter, the stack centred.
const PITCH = 38 * DP;
const DAY_GAP = 6 * DP;
const TOP = (288 - (CELL + DAY_GAP + LABEL_LINE)) / 2;
const DASH_W = 1.5 * DP; // Home's dashed day cell, inset 1dp, dashed 4dp on 3dp off
const DASH_INSET = 1 * DP;
const DASH_ON = 4 * DP;
const DASH_OFF = 3 * DP;
const LIFT = 0.06; // WeekBarRail's drag scale
const LEFT = (1128 - (6 * PITCH + CELL)) / 2;
/** Where each drag enters from: just past the right edge, as if from a tray beside the week. */
const ENTER_X = 1128 + 6 * DP;

// Timeline, in frames. Each drag slides in over TRAVEL frames, drops, then settles; the three are
// spaced so the first leaves on BUILD_IN and the last settles on BUILD_END.
const TRAVEL = 20;
const DROP_LEN = 14;
const SPACING = (BUILD_END - BUILD_IN - TRAVEL - DROP_LEN) / 2;
const DRAGS = [0, 1, 2].map((i) => BUILD_IN + i * SPACING);
/** Frames over which a dropped tile turns from carried (accent) to placed (onBg). */
const SETTLE_COLOUR = 8;

/** Drag `i` at `frame`: its x, its lift (1 carried, 0 settled in the cell), and how placed it is. */
const dragState = (i: number, frame: number) => {
  const start = DRAGS[i];
  const land = start + TRAVEL;
  const t = smoothstep(frame, start, land);
  const x = ENTER_X + (LEFT + PLACED[i].slot * PITCH - ENTER_X) * t;
  const lift = frame < land ? 1 : 1 - settle(frame, land, DROP_LEN);
  const placed = smoothstep(frame, land, land + SETTLE_COLOUR);
  return {x, lift, placed};
};

export const Custom: React.FC = () => {
  const frame = useCurrentFrame();
  const held = heldOut(frame);

  return (
    <AbsoluteFill style={{overflow: 'hidden'}}>
      {DAYS.map((day, i) => {
        const p = PLACED.findIndex((d) => d.slot === i);
        // A day's letter lights, in one step as generated's do, once something has been dropped on it.
        const lit = p >= 0 && (held >= 0.5 || frame >= DRAGS[p] + TRAVEL + 2);
        const x = LEFT + i * PITCH;
        return (
          <React.Fragment key={i}>
            <svg
              width={CELL}
              height={CELL}
              style={{position: 'absolute', left: x, top: TOP}}
            >
              <rect
                x={DASH_INSET}
                y={DASH_INSET}
                width={CELL - 2 * DASH_INSET}
                height={CELL - 2 * DASH_INSET}
                rx={CELL_RADIUS}
                fill="none"
                stroke={withAlpha(MUTED, RUNG.dash)}
                strokeWidth={DASH_W}
                strokeDasharray={`${DASH_ON} ${DASH_OFF}`}
              />
            </svg>
            <div
              style={{
                position: 'absolute',
                left: x,
                width: CELL,
                top: TOP + CELL + DAY_GAP,
                textAlign: 'center',
                fontFamily: MONO,
                fontSize: LABEL_SIZE,
                lineHeight: `${LABEL_LINE}px`,
                ...tracked(LABEL_TRACKING),
                color: lit ? ON_BG : MUTED,
              }}
            >
              {day}
            </div>
          </React.Fragment>
        );
      })}

      {PLACED.map((d, i) => {
        // Held: sitting in its cell, fading out as the week clears.
        if (held > 0) return <Tile key={d.label} label={d.label} x={LEFT + d.slot * PITCH} lift={0} placed={1} alpha={held} />;
        if (frame < DRAGS[i]) return null;
        const {x, lift, placed} = dragState(i, frame);
        return <Tile key={d.label} label={d.label} x={x} lift={lift} placed={placed} alpha={1} />;
      })}
    </AbsoluteFill>
  );
};

/** A training day. [lift] 1 = carried, 0 = set down; [placed] 0 = accent (in hand), 1 = onBg (in the week). */
const Tile: React.FC<{label: string; x: number; lift: number; placed: number; alpha: number}> = ({
  label,
  x,
  lift,
  placed,
  alpha,
}) => (
  <div
    style={{
      position: 'absolute',
      left: x,
      top: TOP,
      width: CELL,
      height: CELL,
      borderRadius: CELL_RADIUS,
      background: mix(ACCENT, ON_BG, placed),
      opacity: alpha,
      transform: `scale(${1 + LIFT * lift})`,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      fontFamily: MONO,
      fontSize: LABEL_SIZE,
      lineHeight: `${LABEL_LINE}px`,
      ...tracked(LABEL_TRACKING),
      color: BG,
    }}
  >
    {label}
  </div>
);
