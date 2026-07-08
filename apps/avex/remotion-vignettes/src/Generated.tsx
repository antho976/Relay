import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {ACCENT_A60, MUTED, ON_BG, OUTLINE_A35} from './theme';

/**
 * "Build me a plan" — the informative version. Beat 1: the three inputs Avex asks for (gear ·
 * goal · days) spring in as pills. Beat 2: they resolve into a real 3-day Push/Pull/Legs week
 * with named exercises and set×rep schemes (reps shaped by the goal — that's the pitch: answer
 * three questions, get a full week). Fades to empty so the loop is seamless.
 *
 * 180 frames @ 30fps = 6s. Videos for the other modes must reuse this length and beat timing.
 */

const INPUT_PILLS = ['FULL GYM', 'BUILD MUSCLE', '3 DAYS'];

const WEEK: {day: string; lines: [string, string][]}[] = [
  {day: 'PUSH', lines: [['Bench press', '3×8'], ['Incline press', '3×10'], ['Lateral raise', '3×12'], ['Pushdown', '3×12']]},
  {day: 'PULL', lines: [['Lat pulldown', '3×10'], ['Seated row', '3×10'], ['Face pull', '3×15'], ['Biceps curl', '3×12']]},
  {day: 'LEGS', lines: [['Squat', '4×6'], ['Leg press', '3×10'], ['Leg curl', '3×12'], ['Calf raise', '3×15']]},
];

// Beat timeline (frames) — retimed slower (240f = 8s) so the pills and the week are comfortably
// readable; the springs stay snappy, only the dwells stretched.
const PILLS_IN = 10;       // first pill enters
const PILL_STAGGER = 8;
const PILLS_OUT = 74;      // pills start leaving
const PILLS_GONE = 88;
const CARDS_IN = 80;       // first day card enters
const CARD_STAGGER = 6;
const LINES_IN = 96;       // first exercise line enters
const LINE_STAGGER = 7;    // per line, sweeping left-to-right across the week
const FADE_START = 222;
const FADE_END = 236;

const GAP = 32;            // 8dp @ 4x
const RADIUS = 16;
const CARD_PAD = 24;

export const Generated: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, width, height} = useVideoConfig();
  const cardW = (width - GAP * (WEEK.length - 1)) / WEEK.length;

  const sceneOpacity = interpolate(frame, [FADE_START, FADE_END], [1, 0], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });
  const pillsLeave = interpolate(frame, [PILLS_OUT, PILLS_GONE], [0, 1], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });

  return (
    <AbsoluteFill style={{opacity: sceneOpacity}}>
      {/* Beat 1 — the inputs. Centered pill row; the goal pill carries the accent. */}
      {pillsLeave < 1 && (
        <AbsoluteFill
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 28,
            opacity: 1 - pillsLeave,
            transform: `translateY(${-28 * pillsLeave}px)`,
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
                  padding: '16px 32px',
                  fontFamily: 'monospace',
                  fontSize: 36,
                  letterSpacing: 3,
                  color: isGoal ? ON_BG : MUTED,
                  opacity: enter,
                  transform: `scale(${0.8 + 0.2 * enter})`,
                }}
              >
                {label}
              </div>
            );
          })}
        </AbsoluteFill>
      )}

      {/* Beat 2 — the week those inputs build. */}
      {WEEK.map((card, c) => {
        const enter = spring({
          frame: frame - (CARDS_IN + c * CARD_STAGGER),
          fps,
          config: {damping: 16, stiffness: 130},
        });
        if (enter <= 0) return null;
        // While this card's lines land, its frame warms toward the accent (the "writing" cue).
        const firstLine = LINES_IN + c * card.lines.length * LINE_STAGGER;
        const lastDone = firstLine + card.lines.length * LINE_STAGGER + 12;
        const writing = interpolate(
          frame,
          [firstLine - 4, firstLine, lastDone, lastDone + 10],
          [0, 1, 1, 0],
          {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'}
        );
        return (
          <div
            key={card.day}
            style={{
              position: 'absolute',
              left: c * (cardW + GAP),
              top: 0,
              width: cardW,
              height,
              borderRadius: RADIUS,
              border: `4px solid ${writing > 0.5 ? ACCENT_A60 : OUTLINE_A35}`,
              padding: CARD_PAD,
              boxSizing: 'border-box',
              opacity: enter,
              transform: `translateY(${(1 - enter) * 24}px)`,
            }}
          >
            <div
              style={{
                fontFamily: 'monospace',
                fontSize: 28,
                letterSpacing: 4,
                color: MUTED,
                marginBottom: 12,
              }}
            >
              {card.day}
            </div>
            {card.lines.map(([name, sets], i) => {
              const lineIn = spring({
                frame: frame - (firstLine + i * LINE_STAGGER),
                fps,
                config: {damping: 15, stiffness: 140},
              });
              return (
                <div
                  key={name}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'baseline',
                    marginBottom: 8,
                    opacity: lineIn,
                    transform: `translateX(${(1 - lineIn) * 16}px)`,
                  }}
                >
                  <span
                    style={{
                      fontFamily: 'sans-serif',
                      fontSize: 32,
                      color: ON_BG,
                      whiteSpace: 'nowrap',
                      overflow: 'hidden',
                    }}
                  >
                    {name}
                  </span>
                  <span
                    style={{
                      fontFamily: 'monospace',
                      fontSize: 26,
                      color: MUTED,
                      marginLeft: 12,
                      flexShrink: 0,
                    }}
                  >
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
