import React from 'react';
import {AbsoluteFill, useCurrentFrame} from 'remotion';
import {ACCENT, bump, DP, heldOut, mix, MONO, MUTED, ON_BG, settle, smoothstep, withAlpha} from './theme';

/**
 * "I'll make my own" — you decide what goes on which day.
 *
 * The week is laid out empty, seven dashed slots, and a finger DRAGS each training day into the slot
 * it chose: PUSH onto Monday, PULL onto Wednesday, LEGS onto Friday. Dragging is the one gesture no
 * other card has, and it is the whole difference between this option and the generated one: the
 * same week, but every day in it was put there by hand. The days you leave empty stay dashed, so the
 * frozen frame still reads as a week you could keep filling.
 *
 * Frame 0 and the last frame are the same three placed days, so the seam and the freeze land on it.
 */

const PLACED = [
  {slot: 0, label: 'PUSH'},
  {slot: 2, label: 'PULL'},
  {slot: 4, label: 'LEGS'},
];
const DAYS = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];

// Geometry, in dp on the 282×72 strip.
const SLOT_W = 34 * DP;
const SLOT_H = 40 * DP;
const PITCH = 38 * DP;
const TOP = 8.5 * DP;
const RADIUS = 8 * DP;
const DASH = 0.75 * DP;
const DAY_GAP = 5 * DP;
const DAY_SIZE = 10 * DP; // labelS
const CHIP_SIZE = 9 * DP;
const TOUCH_R = 9 * DP;
const LEFT = (1128 - (6 * PITCH + SLOT_W)) / 2;
/** Where each drag enters from: just past the right edge, as if from a tray beside the week. */
const ENTER_X = 1128 + 6 * DP;

// Timeline, in frames. Each drag slides in over TRAVEL frames, drops, then settles.
const DRAGS = [42, 68, 94];
const TRAVEL = 20;
const DROP_LEN = 14;

/** Drag `i` at `frame`: its x, and its lift (1 carried, 0 settled in the slot). */
const dragState = (i: number, frame: number) => {
  const start = DRAGS[i];
  const t = smoothstep(frame, start, start + TRAVEL);
  const x = ENTER_X + (LEFT + PLACED[i].slot * PITCH - ENTER_X) * t;
  const lift = frame < start + TRAVEL ? 1 : 1 - settle(frame, start + TRAVEL, DROP_LEN);
  return {x, lift};
};

export const Custom: React.FC = () => {
  const frame = useCurrentFrame();
  const held = heldOut(frame);

  return (
    <AbsoluteFill style={{overflow: 'hidden'}}>
      {DAYS.map((day, i) => {
        const p = PLACED.findIndex((d) => d.slot === i);
        // A day's letter lights once something has been dropped on it.
        const filled = p < 0 ? 0 : Math.max(held, smoothstep(frame, DRAGS[p] + TRAVEL, DRAGS[p] + TRAVEL + 4));
        const x = LEFT + i * PITCH;
        return (
          <React.Fragment key={i}>
            <div
              style={{
                position: 'absolute',
                left: x,
                top: TOP,
                width: SLOT_W,
                height: SLOT_H,
                boxSizing: 'border-box',
                borderRadius: RADIUS,
                border: `${DASH}px dashed ${withAlpha(MUTED, 0.35)}`,
              }}
            />
            <div
              style={{
                position: 'absolute',
                left: x,
                width: SLOT_W,
                top: TOP + SLOT_H + DAY_GAP,
                textAlign: 'center',
                fontFamily: MONO,
                fontSize: DAY_SIZE,
                lineHeight: `${DAY_SIZE}px`,
                color: mix(MUTED, ON_BG, filled),
              }}
            >
              {day}
            </div>
          </React.Fragment>
        );
      })}

      {PLACED.map((d, i) => {
        // Held: sitting in its slot, fading out as the week clears.
        if (held > 0) return <Chip key={d.label} label={d.label} x={LEFT + d.slot * PITCH} lift={0} alpha={held} />;
        if (frame < DRAGS[i]) return null;
        const {x, lift} = dragState(i, frame);
        return <Chip key={d.label} label={d.label} x={x} lift={lift} alpha={1} />;
      })}

      {/* The finger: Android's "show taps" dot, riding each tile until it is dropped. */}
      {PLACED.map((d, i) => {
        const start = DRAGS[i];
        const a = Math.min(smoothstep(frame, start, start + 4), 1 - smoothstep(frame, start + TRAVEL + 1, start + TRAVEL + 7));
        if (a <= 0) return null;
        const {x} = dragState(i, frame);
        const r = TOUCH_R * (1 - 0.15 * bump(frame, start + TRAVEL - 2, 8));
        return (
          <div
            key={d.label}
            style={{
              position: 'absolute',
              left: x + SLOT_W / 2 - r,
              top: TOP + SLOT_H * 0.62 - r,
              width: r * 2,
              height: r * 2,
              borderRadius: '50%',
              background: withAlpha(ON_BG, 0.35 * a),
            }}
          />
        );
      })}
    </AbsoluteFill>
  );
};

/** A training day: an accent tile with its split. [lift] 1 = carried (bigger, tilted), 0 = placed. */
const Chip: React.FC<{label: string; x: number; lift: number; alpha: number}> = ({label, x, lift, alpha}) => (
  <div
    style={{
      position: 'absolute',
      left: x,
      top: TOP,
      width: SLOT_W,
      height: SLOT_H,
      borderRadius: RADIUS,
      background: ACCENT,
      opacity: alpha,
      transform: `scale(${1 + 0.1 * lift}) rotate(${-5 * lift}deg)`,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      fontFamily: MONO,
      fontSize: CHIP_SIZE,
      lineHeight: `${CHIP_SIZE}px`,
      color: ON_BG,
    }}
  >
    {label}
  </div>
);
