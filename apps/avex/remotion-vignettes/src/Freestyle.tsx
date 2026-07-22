import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {ACCENT, MUTED, ON_BG} from './theme';

/**
 * "Go with the flow" — no plan, no frame. The loop is SEAMLESS: frame 0 and the final frame are BOTH
 * the same finished log, held still, so the restart never jumps. In between the entries clear and you
 * re-log them one at a time — scattered across the strip, in no order, at an irregular "whenever"
 * rhythm — each dropping in with the accent tick (the "logged it" mark). Where the generated video
 * fills a week and the custom one hand-builds a day, this one just catches what you actually did: a
 * mixed bag (a run in among the lifts), with nothing boxing it in. Plays twice then FREEZES on the log.
 *
 * Shares Generated/Custom's length + held-first / held-last shape so all three cards start, loop, and
 * freeze together. No day frame — the missing structure IS the point (mirrors the Canvas twin's scatter
 * in PlanModeVignettes.drawFreestyle).
 */

// name · sets · a loose (left, center-y) fraction of the strip · the frame it re-logs on. Deliberately
// NOT a grid, and a run mixed in with the lifts (freestyle = whatever you did). Positions echo the
// Canvas twin so the decode/reduce-motion fallback and the video read as the same illustration.
type Log = {name: string; sets: string; x: number; y: number; in: number};
const LOGS: Log[] = [
  {name: 'Curls', sets: '3×12', x: 0.34, y: 0.52, in: 60},
  {name: 'Bench', sets: '4×8', x: 0.05, y: 0.2, in: 78},
  {name: 'Squat', sets: '5×5', x: 0.08, y: 0.84, in: 104},
  {name: 'Run', sets: '2 km', x: 0.58, y: 0.16, in: 132},
  {name: 'Pull-ups', sets: '3×10', x: 0.66, y: 0.64, in: 170},
];

// Phase timeline (frames @ 30fps). Opens HELD on the log, clears it, then re-logs at irregular gaps
// (the "whenever" rhythm — no even stagger like the planned modes), and closes HELD on the same log.
const CLEAR_START = 34;
const CLEAR_END = 50;

const ROW_H = 44;

/** 1 while the opening log is held, 1→0 as it clears, 0 thereafter. */
const heldOut = (frame: number): number =>
  interpolate(frame, [CLEAR_START, CLEAR_END], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});

export const Freestyle: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, width, height} = useVideoConfig();
  const held = heldOut(frame);

  return (
    <AbsoluteFill>
      {LOGS.map((log) => {
        // appear: 1 held, 0 cleared, springs back to 1 on re-log, 1 held again — frame 0 and the last
        // frame both land on 1, so the loop seam and the freeze are the same finished log.
        const relog = spring({frame: frame - log.in, fps, config: {damping: 12, stiffness: 150}});
        const appear = Math.min(1, held + relog);
        if (appear <= 0) return null;
        // Pop-then-settle overshoot on the fresh re-log (not during either hold), matching the Canvas twin.
        const pop = held > 0 ? 1 : 1 + 0.1 * Math.sin(Math.min(1, relog) * Math.PI);
        return (
          <div
            key={log.name}
            style={{
              position: 'absolute',
              left: width * log.x,
              top: height * log.y - ROW_H / 2,
              height: ROW_H,
              display: 'flex',
              alignItems: 'center',
              opacity: appear,
              transform: `translateY(${(1 - appear) * 12}px) scale(${pop})`,
            }}
          >
            {/* accent tick = the work you did (shared with the custom video's "the work" mark) */}
            <div style={{width: 14, height: 14, borderRadius: 4, background: ACCENT, marginRight: 16, flexShrink: 0}} />
            <span style={{fontFamily: 'sans-serif', fontSize: 33, color: ON_BG, whiteSpace: 'nowrap'}}>{log.name}</span>
            <span style={{fontFamily: 'monospace', fontSize: 27, color: MUTED, marginLeft: 12, flexShrink: 0}}>
              {log.sets}
            </span>
          </div>
        );
      })}
    </AbsoluteFill>
  );
};
