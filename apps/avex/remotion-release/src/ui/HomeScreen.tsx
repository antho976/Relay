import React from 'react';
import {interpolate} from 'remotion';
import {BottomBar, Col, Cta, Gut, GoalRow, Row, Screen, StatusBar, Txt, WeekStrip, useK} from './kit';
import {Icon} from './icons';
import {C, type} from './tokens';

const mix = (t: number, a: string, b: string) => {
  const p = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
  const [r1, g1, b1] = p(a); const [r2, g2, b2] = p(b);
  const c = (x: number, y: number) => Math.round(x + (y - x) * t);
  return `rgb(${c(r1, r2)}, ${c(g1, g2)}, ${c(b1, b2)})`;
};

/** Cross-fade two things in the same box, so a change reads as one element becoming another. */
const Swap: React.FC<{t: number; a: React.ReactNode; b: React.ReactNode; style?: React.CSSProperties}> = ({
  t, a, b, style,
}) => {
  // The two states must never both be legible: crossfading them 50/50 double-exposes two different
  // sentences in the same box, which is the one thing a morph must not do. The old one is gone
  // before the new one arrives, with a beat of nothing between.
  const out = Math.max(0, Math.min(1, (0.44 - t) / 0.44));
  const inn = Math.max(0, Math.min(1, (t - 0.56) / 0.44));
  return (
    <div style={{position: 'relative', ...style}}>
      <div style={{opacity: out, transform: `translateY(${(1 - out) * -7}px)`}}>{a}</div>
      <div style={{position: 'absolute', inset: 0, opacity: inn, transform: `translateY(${(1 - inn) * 7}px)`}}>{b}</div>
    </div>
  );
};

/**
 * Home, drawn rather than filmed, with `t` walking 0.8.9 → 0.9. Because both eras are one component,
 * every change is a tween instead of a cut: the wordmark becomes the bell, the pill becomes the
 * accent CTA, the meters bleed navy to red — all on the same clock.
 */
export const HomeScreen: React.FC<{width: number; t: number; goalFill?: number; badge?: string}> = ({
  width, t, goalFill = 1, badge = '9+',
}) => {
  const accent = mix(t, C.accentOld, C.accent);
  return (
    <Screen width={width}>
      <Inner t={t} accent={accent} goalFill={goalFill} badge={badge} />
    </Screen>
  );
};

const Inner: React.FC<{t: number; accent: string; goalFill: number; badge: string}> = ({t, accent, goalFill, badge}) => {
  const k = useK();
  const G = [
    {name: 'Incline Barbell Bench', cur: 150, target: 160, fill: 0.94, glyph: 'barbell' as const},
    {name: 'Hack Squat', cur: 240, target: 260, fill: 0.92, glyph: 'machine' as const},
    {name: 'Barbell Row', cur: 160, target: 175, fill: 0.91, glyph: 'barbell' as const},
  ];
  return (
    <>
      <StatusBar />
      {/* top bar */}
      <Gut style={{marginTop: 10 * k}}>
        <Row style={{justifyContent: 'space-between', height: 44 * k}}>
          <Swap
            t={t}
            a={<div style={{...type('titleMedium', k, C.muted), fontStyle: 'italic'}}>• Avex</div>}
            b={
              <div style={{position: 'relative'}}>
                <Icon name="bell" size={21 * k} color={C.muted} />
                <div
                  style={{
                    position: 'absolute', top: -6 * k, left: 8 * k, background: accent, borderRadius: 999,
                    padding: `${1 * k}px ${4 * k}px`, ...type('labelSmall', k, C.onBg),
                  }}
                >
                  {badge}
                </div>
              </div>
            }
            style={{width: 70 * k, height: 26 * k}}
          />
          <Row style={{gap: 14 * k}}>
            <div style={{opacity: t}}><Icon name="profile" size={20 * k} color={C.muted} /></div>
            <Icon name="gear" size={20 * k} color={C.muted} />
          </Row>
        </Row>
      </Gut>

      {/* hero */}
      <Gut style={{marginTop: 12 * k}}>
        <Txt v="TODAY" t="labelMedium" color={C.muted} />
        <Txt v="Upper A" t="displayLarge" style={{marginTop: 4 * k}} />
        <Swap
          t={t}
          a={
            <div style={{...type('bodyMedium', k, C.muted), fontStyle: 'italic', marginTop: 6 * k}}>
              Push-leaning · Size focus · 6 exercises
            </div>
          }
          b={
            <Col style={{marginTop: 6 * k, gap: 3 * k}}>
              <div style={type('bodyMedium', k, C.muted)}>It&apos;s been 3 days since your last session.</div>
              <div style={type('labelMedium', k, C.muted)}>Incline Barbell Bench · 4×6-10 @ 150 lb</div>
              <div style={type('labelMedium', k, C.muted)}>DB Row (1-arm) · 4×6-10 @ 80 lb</div>
            </Col>
          }
          style={{height: 52 * k}}
        />
        <Row style={{marginTop: 16 * k, gap: 10 * k}}>
          <div style={{transform: `scale(${1})`}}>
            <Cta label={t > 0.5 ? 'Start session' : 'Start session →'} era={t > 0.5 ? 'new' : 'old'} width={t > 0.5 ? 232 : 0 || undefined} accent={accent} />
          </div>
          <div style={{opacity: interpolate(t, [0.45, 0.85], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'})}}>
            <div
              style={{
                width: 66 * k, height: 56 * k, borderRadius: 12 * k, border: `1.4px solid ${C.outline}`,
                display: 'flex', alignItems: 'center', justifyContent: 'center', ...type('labelLarge', k, C.onBg),
              }}
            >
              Plan
            </div>
          </div>
        </Row>
        <div style={{...type('labelSmall', k, C.muted), marginTop: 7 * k}}>
          {t > 0.5 ? 'Hold start to skip warmup' : 'Hold to skip warmup'}
        </div>
      </Gut>

      {/* this week */}
      <Gut style={{marginTop: 22 * k}}>
        <Row style={{justifyContent: 'space-between', marginBottom: 10 * k}}>
          <Txt v="THIS WEEK" t="labelLarge" color={C.muted} />
          <Swap
            t={t}
            a={<div style={type('labelMedium', k, C.muted)}>0 of 4 target · view program →</div>}
            b={<div style={type('labelMedium', k, C.muted)}>0 / 4 days</div>}
            style={{height: 14 * k, width: 150 * k, textAlign: 'right'}}
          />
        </Row>
        <WeekStrip filled={t > 0.5 ? [0] : []} accent={accent} era={t > 0.5 ? 'new' : 'old'} />
      </Gut>

      {/* goals */}
      <Gut style={{marginTop: 24 * k}}>
        <Row style={{justifyContent: 'space-between', marginBottom: 12 * k}}>
          <Txt v="GOALS" t="labelLarge" color={C.muted} />
          <div style={type('labelMedium', k, C.muted)}>view all →</div>
        </Row>
        {G.map((g) => (
          <GoalRow
            key={g.name} name={g.name} cur={g.cur} target={g.target}
            fill={g.fill * goalFill} accent={accent} chip={t > 0.5} glyph={g.glyph}
          />
        ))}
      </Gut>

      <BottomBar
        items={[
          {label: 'Cardio', icon: 'cardio'},
          {label: 'Stats', icon: 'stats'},
          {label: 'Home', icon: 'home'},
          {label: 'Coach', icon: 'coach'},
          t > 0.5 ? {label: 'Academy', icon: 'academy'} : {label: 'Profile', icon: 'profile'},
        ]}
        active={2} accent={accent} badge={t > 0.5 ? 9 : undefined}
      />
    </>
  );
};
