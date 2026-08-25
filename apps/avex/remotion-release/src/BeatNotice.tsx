import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {HomeScreen} from './ui/HomeScreen';
import {Device} from './Device';
import {Icon} from './ui/icons';
import {C, type as tokenType} from './ui/tokens';
import {EASE} from './Camera';
import {Plate, Split} from './Layout';
import {Cue} from './Sound';
import {Body, Eyebrow, Title, useEdgeFade} from './Type';
import {ACCENT, MONO, MUTED, ON_BG, SERIF, SHOT_AR} from './theme';

/**
 * The bell. `ui/notifications/` did not exist at 0.8.9 — Home opened with up to four stacked banner
 * strips, and 0.9 replaces every one of them with a single feed behind a bell that carries a live
 * unread count.
 *
 * Drawn, not filmed, for a specific reason: the behaviour worth showing is that a newly unlocked
 * lesson *never interrupts*. It settles as a banner over whatever is on screen, then flies into the
 * bell and bumps the count. A screen recording of that is four frames of something small moving; at
 * this size it is the whole point of the feature.
 */

const PHONE_H = 940;
const PHONE_W = PHONE_H / SHOT_AR;
const SCALE = PHONE_W / 384;      // dp → px at this phone size
const BELL = {x: 34 * SCALE + 10, y: 34 * SCALE + 10 * SCALE + 13 * SCALE};

export const NoticeBeat: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 10);

  const at = (f: number) => Math.round(durationInFrames * f);
  const IN = at(0.18);
  const FLY = at(0.58);
  const LAND = FLY + 14;

  const drop = spring({frame: frame - IN, fps, config: {damping: 200, stiffness: 90}});
  const fly = interpolate(frame, [FLY, LAND], [0, 1], {
    extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.rush,
  });
  const landed = frame >= LAND;
  const pop = spring({frame: frame - LAND, fps, config: {damping: 9, stiffness: 220}});

  // Banner start: centred near the top of the screen. End: the bell itself.
  const startX = PHONE_W / 2;
  const startY = 108;
  const x = startX + (BELL.x - startX) * fly;
  const y = startY + (BELL.y - startY) * fly;
  const scale = (0.94 + drop * 0.06) * (1 - fly * 0.93);

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        {/* the banner arriving, the flight itself, and the count acknowledging it */}
        <Cue at={IN + 3} sfx="pop" gain={0.8} />
        <Cue at={FLY - 1} sfx="swoosh" />
        <Cue at={LAND} sfx="ding" />

        <Split
          flip
          copyWidth={640}
          copy={
            <>
              <Eyebrow delay={0}>New in 0.9</Eyebrow>
              <Title delay={4} size={68}>{'All your alerts\nin one place'}</Title>
              <Body delay={10} width={600}>
                Home used to open with up to four banners stacked on top of each other. Now everything
                lands in one feed behind the bell. Unlock a lesson mid-session and it slides in
                quietly, waits a moment, then tucks itself away.
              </Body>
            </>
          }
        >
          <Device height={PHONE_H}>
            <HomeScreen width={PHONE_W} t={1} goalFill={1} badge={landed ? '10' : '9'} />

            {/* the count acknowledging the arrival */}
            {landed ? (
              <div
                style={{
                  position: 'absolute', left: BELL.x - 26, top: BELL.y - 26, width: 52, height: 52,
                  borderRadius: 999, border: `2px solid ${ACCENT}`, pointerEvents: 'none',
                  transform: `scale(${1 + pop * 0.5})`, opacity: (1 - pop) * 0.85,
                }}
              />
            ) : null}

            {drop > 0.01 && fly < 0.999 ? (
              <div
                style={{
                  position: 'absolute', left: x, top: y, width: PHONE_W - 58, marginLeft: -(PHONE_W - 58) / 2,
                  transform: `translateY(${(1 - drop) * -70}px) scale(${scale})`,
                  transformOrigin: 'center center',
                  opacity: drop * (1 - Math.max(0, fly - 0.72) / 0.28),
                }}
              >
                <ArrivalBanner />
              </div>
            ) : null}
          </Device>
        </Split>
      </Plate>
    </AbsoluteFill>
  );
};

/** The ArrivalBanner, at the proportions it has on device. */
const ArrivalBanner: React.FC = () => (
  <div
    style={{
      background: C.surfaceVar, border: `1px solid ${C.outline}`, borderRadius: 14,
      padding: '13px 15px', display: 'flex', gap: 13, alignItems: 'center',
      boxShadow: '0 18px 42px rgba(0,0,0,0.7)',
    }}
  >
    <div
      style={{
        width: 34, height: 34, borderRadius: 9, background: `${ACCENT}1F`,
        display: 'flex', alignItems: 'center', justifyContent: 'center', flex: '0 0 auto',
      }}
    >
      <Icon name="academy" size={18} color={ACCENT} />
    </div>
    <div style={{display: 'flex', flexDirection: 'column', gap: 3, minWidth: 0}}>
      <div style={{fontFamily: MONO, fontSize: 9.5, letterSpacing: 1.4, color: ACCENT}}>NEW LESSON</div>
      <div style={{fontFamily: SERIF, fontSize: 15, color: ON_BG, lineHeight: 1.2}}>
        Form first, load second
      </div>
      <div style={{...tokenType('bodySmall', 1, MUTED), fontSize: 11}}>Unlocked by your last session</div>
    </div>
  </div>
);
