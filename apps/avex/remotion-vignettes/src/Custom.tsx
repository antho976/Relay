import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {ACCENT, MUTED, ON_BG, OUTLINE_A35} from './theme';

/**
 * "I'll make my own" — YOU build it. The loop is SEAMLESS: frame 0 and the final frame are BOTH the
 * finished day, held still, so the restart never jumps. In between the exercises clear and you re-add
 * them one at a time — each drops in with an accent tick, and the "+ Add exercise" affordance slides
 * down to open the next slot. Where the generated video fills a whole week, this fills ONE day, by
 * hand. The card plays this twice then FREEZES on the day you built.
 *
 * Shares Generated.tsx's length + held-first / held-last shape so the two cards start, loop, and
 * freeze together.
 */

const ROWS: [string, string][] = [
  ['Bench press', '3×8'],
  ['Overhead press', '3×10'],
  ['Dips', '3×12'],
  ['Triceps pushdown', '3×15'],
];

// Phase timeline (frames @ 30fps). The day frame + label are constant (the container never clears);
// only the exercises clear and re-add, so frame 0 and the last frame are the same finished day.
const CLEAR_START = 34; // the opening exercises start to clear
const CLEAR_END = 50;
const ROW_IN = [58, 90, 122, 154]; // each exercise you re-add
const SLIDE_LEAD = 7; // the "+ add" affordance opens the next slot just ahead of the row

const RADIUS = 16;
const PAD = 22;
const FRAME_FRAC = 0.66; // single centered day, narrower than the full-width week

/** 1 while the opening exercises are held, 1→0 as they clear, 0 thereafter. */
const heldOut = (frame: number): number =>
  interpolate(frame, [CLEAR_START, CLEAR_END], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});

export const Custom: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, width, height, durationInFrames} = useVideoConfig();

  const frameW = width * FRAME_FRAC;
  const left = (width - frameW) / 2;
  const innerW = frameW - PAD * 2;
  const held = heldOut(frame);

  const labelH = 48;
  const slots = ROWS.length + 1; // rows + the trailing "+ add" affordance
  const rowH = (height - PAD * 2 - labelH) / slots;
  const rowTop = (slot: number) => PAD + labelH + slot * rowH;

  // Each row's presence: 1 held, 0 cleared, springs back to 1 on re-add, 1 held again.
  const rowAppear = ROW_IN.map((t) => Math.min(1, held + spring({frame: frame - t, fps, config: {damping: 16, stiffness: 160}})));
  // The "+ add" affordance sits at the first open slot = however many rows are currently present, and
  // leads each re-add slightly so it never shares a slot with an incoming row. At both holds → slot 4.
  const ghostSlot = ROW_IN.reduce((acc, t) => acc + Math.min(1, held + reveal(frame, t - SLIDE_LEAD, 10)), 0);
  // Whole number of pulse cycles across the loop so the affordance's opacity matches at the seam.
  const ghostPulse = 0.55 + 0.3 * Math.sin((frame / durationInFrames) * 2 * Math.PI * 7);
  // Hide the affordance across the clear→empty window so it fades out WITH the rows instead of
  // sliding up through them, then fades back in at the top to start the re-add.
  const ghostGate = interpolate(frame, [CLEAR_START, CLEAR_START + 8, CLEAR_END + 6, CLEAR_END + 14], [1, 0, 0, 1], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });

  return (
    <AbsoluteFill>
      <div
        style={{
          position: 'absolute',
          left,
          top: 0,
          width: frameW,
          height,
          borderRadius: RADIUS,
          border: `4px solid ${OUTLINE_A35}`,
          padding: PAD,
          boxSizing: 'border-box',
        }}
      >
        {/* Day label — the constant container. */}
        <div style={{display: 'flex', alignItems: 'center', gap: 12}}>
          <span style={{fontFamily: 'monospace', fontSize: 27, letterSpacing: 5, color: MUTED}}>PUSH</span>
          <div style={{flex: 1, height: 2, background: OUTLINE_A35}} />
        </div>

        {/* Exercises — held, cleared, then re-added one at a time. */}
        {ROWS.map(([name, sets], i) => {
          const appear = rowAppear[i];
          if (appear <= 0) return null;
          return (
            <div
              key={name}
              style={{
                position: 'absolute',
                left: PAD,
                top: rowTop(i),
                width: innerW,
                height: rowH,
                display: 'flex',
                alignItems: 'center',
                opacity: appear,
                transform: `translateX(${(1 - appear) * 16}px)`,
              }}
            >
              {/* accent tick = the work you chose */}
              <div style={{width: 12, height: 12, borderRadius: 3, background: ACCENT, marginRight: 16, flexShrink: 0}} />
              <span
                style={{fontFamily: 'sans-serif', fontSize: 33, color: ON_BG, whiteSpace: 'nowrap', overflow: 'hidden', flex: 1}}
              >
                {name}
              </span>
              <span style={{fontFamily: 'monospace', fontSize: 27, color: MUTED, marginLeft: 12, flexShrink: 0}}>
                {sets}
              </span>
            </div>
          );
        })}

        {/* The "+ Add exercise" affordance — always at the next open slot, gently pulsing. */}
        <div
          style={{
            position: 'absolute',
            left: PAD,
            top: rowTop(ghostSlot),
            width: innerW,
            height: rowH,
            display: 'flex',
            alignItems: 'center',
            opacity: ghostPulse * ghostGate,
          }}
        >
          <div
            style={{
              width: 26,
              height: 26,
              borderRadius: 999,
              border: `2px solid ${ACCENT}`,
              color: ACCENT,
              fontFamily: 'sans-serif',
              fontSize: 26,
              lineHeight: '22px',
              textAlign: 'center',
              marginRight: 16,
              flexShrink: 0,
            }}
          >
            +
          </div>
          <span style={{fontFamily: 'sans-serif', fontSize: 30, color: MUTED}}>Add exercise</span>
        </div>
      </div>
    </AbsoluteFill>
  );
};

/** 0 before start, smoothstep 0→1 across [start, start+ramp], 1 after. */
function reveal(frame: number, start: number, ramp: number): number {
  const t = Math.min(1, Math.max(0, (frame - start) / ramp));
  return t * t * (3 - 2 * t);
}
