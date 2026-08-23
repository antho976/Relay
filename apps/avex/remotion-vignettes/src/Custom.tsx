import React from 'react';
import {AbsoluteFill, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {LABEL_SIZE, MonoLabel, SET_H, SetRow, setsWidth} from './Marks';
import {ACCENT, CLEAR_END, heldOut, HOLD_END, LOOP_FRAMES, MUTED, smoothstep} from './theme';

/**
 * "I'll make my own" — one day, and you name every exercise in it.
 *
 * The text is the tell. Where the generated card is dated days and their splits — a WEEK, handed
 * over — this one is BENCH · INCLINE · DIPS: the actual exercises, which is the level you work at
 * when you build a plan yourself. One day, close up.
 *
 * And it never finishes. Rows land one at a time, slowly enough to watch a decision get made, and
 * the video freezes with `+ ADD` still blinking on the next open line. That is the honest end state
 * for this option: a plan that is yours isn't done until you say it is.
 *
 * Shares Generated's length and its held-first / held-last shape so the cards stay in lockstep.
 */

/** One hand-built push day. The blocks are that exercise's sets. */
const ROWS = [
  {name: 'BENCH', sets: 3},
  {name: 'INCLINE', sets: 4},
  {name: 'DIPS', sets: 3},
];

/** One row per beat — a full second apart, against the generated card's near-simultaneous snap. */
const ROW_IN = [46, 76, 106];
const CURSOR_TAP = -6; // it taps, then drops to the next line just ahead of the row it placed
const CURSOR_TRAVEL = 9;

const ROW_PITCH = 62;
const COL_SETS = 300; // x of the first set block, past the longest name
const CURSOR_R = 19;

export const Custom: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, width, height} = useVideoConfig();
  const held = heldOut(frame);

  const blockW = COL_SETS + setsWidth(Math.max(...ROWS.map((r) => r.sets)));
  const left = (width - blockW) / 2;
  // Rows 0..n-1 plus the open line the `+` waits on.
  const top = (height - (LABEL_SIZE + ROWS.length * ROW_PITCH)) / 2;
  const lineY = (line: number) => top + line * ROW_PITCH;

  // Each row: 1 held, 0 cleared, springs back to 1 when you add it, 1 held again — so frame 0 and the
  // last frame are the same day and the seam never jumps.
  const placed = ROW_IN.map((t) =>
    Math.min(1, held + spring({frame: frame - t, fps, config: {damping: 17, stiffness: 170}}))
  );

  // The cursor waits on the first open line, dropping once a row has landed on the one above it.
  const stepped = ROW_IN.reduce(
    (acc, t) => acc + smoothstep(frame, t + CURSOR_TAP, t + CURSOR_TAP + CURSOR_TRAVEL),
    0
  );
  // Before the clear it is already parked on the last line; it is invisible across the clear itself,
  // so jumping back to the top there costs nothing.
  const cursorLine = frame < CLEAR_END ? ROWS.length : stepped;
  const cursorAlpha = Math.max(
    1 - smoothstep(frame, HOLD_END, HOLD_END + 8),
    smoothstep(frame, CLEAR_END, CLEAR_END + 10)
  );
  // A whole number of cycles across the loop so the blink matches at the seam, phased so BOTH holds —
  // including the frame the card freezes on — catch it at full strength.
  const blink = 0.55 + 0.45 * Math.cos((frame / LOOP_FRAMES) * 2 * Math.PI * 5);
  const press =
    1 - 0.16 * ROW_IN.reduce((m, t) => Math.max(m, Math.max(0, 1 - Math.abs(frame - (t + CURSOR_TAP)) / 6)), 0);

  return (
    <AbsoluteFill>
      {ROWS.map((row, i) => {
        const appear = placed[i];
        if (appear <= 0) return null;
        // Fresh placements bloom and settle; neither hold glows.
        const glow = held > 0 ? 0 : 1 - smoothstep(frame, ROW_IN[i], ROW_IN[i] + 20);
        return (
          <div
            key={row.name}
            style={{
              position: 'absolute',
              left,
              top: lineY(i),
              opacity: appear,
              transform: `translateX(${(1 - appear) * 18}px)`,
            }}
          >
            <MonoLabel opacity={1}>{row.name}</MonoLabel>
            <div style={{position: 'absolute', left: COL_SETS, top: (LABEL_SIZE - SET_H) / 2}}>
              <SetRow sets={row.sets} alphaAt={() => 1} glow={glow} />
            </div>
          </div>
        );
      })}

      {/* `+ ADD` — you, on the next open line. The one row the generated card hasn't got. */}
      <div
        style={{
          position: 'absolute',
          left,
          top: lineY(cursorLine),
          height: LABEL_SIZE,
          display: 'flex',
          alignItems: 'center',
          gap: 18,
          opacity: cursorAlpha * blink,
        }}
      >
        <div
          style={{
            position: 'relative',
            width: CURSOR_R * 2,
            height: CURSOR_R * 2,
            borderRadius: 999,
            border: `3px solid ${ACCENT}`,
            transform: `scale(${press})`,
          }}
        >
          <div style={plusBar(CURSOR_R, false)} />
          <div style={plusBar(CURSOR_R, true)} />
        </div>
        <MonoLabel opacity={1} color={MUTED}>
          ADD
        </MonoLabel>
      </div>
    </AbsoluteFill>
  );
};

/**
 * Bars of length `r`, centred on the ring. Centred by translation rather than by offsetting from the
 * box's edges: absolutely positioned children sit in the PADDING box, which under `border-box` sizing
 * is the ring's 3px stroke smaller than `r * 2` and inset by it — edge maths against `r * 2` put the
 * plus exactly one border-width down and to the right. The padding box is concentric with the ring
 * either way, so centring in it is correct under both.
 */
const plusBar = (r: number, vertical: boolean): React.CSSProperties => ({
  position: 'absolute',
  left: '50%',
  top: '50%',
  transform: 'translate(-50%, -50%)',
  width: vertical ? 4 : r,
  height: vertical ? r : 4,
  borderRadius: 2,
  background: ACCENT,
});
