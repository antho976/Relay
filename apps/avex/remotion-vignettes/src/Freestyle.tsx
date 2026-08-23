import React from 'react';
import {AbsoluteFill, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {LABEL_SIZE, MonoLabel, SetRow} from './Marks';
import {heldOut, smoothstep} from './theme';

/**
 * "Go with the flow" — no plan to fill in, just what you actually did.
 *
 * The other two cards are columns; this one has no column, no rail and no order. The same set blocks
 * land wherever there is room, at uneven intervals, each stamped with the day it happened on — and
 * those days run WED · TUE · SAT · MON · FRI · THU, deliberately out of sequence. The missing structure is
 * the message: the absence of a grid is doing the same job the grid does on the cards above. The
 * intervals between landings are uneven for the same reason — a plan has a tempo, this doesn't.
 *
 * Shares Generated/Custom's length and its held-first / held-last shape so all three cards start,
 * loop and freeze together, and the freeze lands on a full log — proof the mode keeps a record.
 */

/**
 * Laid out by hand (px on the 1128×288 canvas). No two entries share a row or a left edge and the
 * vertical gaps are all different, so nothing lines up into an accidental grid — the moment two of
 * these agree on an axis the card starts arguing for the option above it instead of its own.
 */
type Log = {day: string; sets: number; x: number; y: number; in: number};
const LOGS: Log[] = [
  {day: 'WED', sets: 4, x: 30, y: 18, in: 42},
  {day: 'TUE', sets: 4, x: 700, y: 150, in: 52},
  {day: 'SAT', sets: 3, x: 44, y: 200, in: 74},
  {day: 'MON', sets: 5, x: 560, y: 46, in: 86},
  {day: 'FRI', sets: 4, x: 420, y: 226, in: 104},
  {day: 'THU', sets: 3, x: 132, y: 104, in: 116},
];

const STAMP_GAP = 18; // day stamp → its sets

export const Freestyle: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const held = heldOut(frame);

  return (
    <AbsoluteFill>
      {LOGS.map((log) => {
        // 1 held, 0 cleared, springs back to 1 when it is logged, 1 held again — frame 0 and the last
        // frame are the same log, so the seam and the freeze land on it.
        const relog = spring({frame: frame - log.in, fps, config: {damping: 12, stiffness: 160}});
        const appear = Math.min(1, held + relog);
        if (appear <= 0) return null;
        // A pop that settles, on the fresh landing only — never during either hold.
        const pop = held > 0 ? 1 : 1 + 0.11 * Math.sin(Math.min(1, relog) * Math.PI);
        const glow = held > 0 ? 0 : 1 - smoothstep(frame, log.in, log.in + 20);
        return (
          <div
            key={log.day}
            style={{
              position: 'absolute',
              left: log.x,
              top: log.y,
              height: LABEL_SIZE,
              display: 'flex',
              alignItems: 'center',
              gap: STAMP_GAP,
              opacity: appear,
              transform: `scale(${pop})`,
              transformOrigin: 'left center',
            }}
          >
            <MonoLabel opacity={1}>{log.day}</MonoLabel>
            <SetRow sets={log.sets} alphaAt={() => 1} glow={glow} />
          </div>
        );
      })}
    </AbsoluteFill>
  );
};
