import React from 'react';
import {Icon, IconName} from './icons';
import {C, GUTTER, SCREEN_DP, type} from './tokens';

/** dp→px scale, so every component below is authored in the app's own units. */
export const ScaleCtx = React.createContext(1);
export const useK = () => React.useContext(ScaleCtx);

export const Screen: React.FC<{
  children: React.ReactNode; width: number; radius?: number; style?: React.CSSProperties;
}> = ({children, width, radius = 34, style}) => {
  const k = width / SCREEN_DP.w;
  return (
    <ScaleCtx.Provider value={k}>
      <div
        style={{
          width, height: SCREEN_DP.h * k, background: C.bg, borderRadius: radius * k,
          overflow: 'hidden', position: 'relative', flex: '0 0 auto', ...style,
        }}
      >
        {children}
      </div>
    </ScaleCtx.Provider>
  );
};

export const Row: React.FC<{children: React.ReactNode; style?: React.CSSProperties}> = ({children, style}) => (
  <div style={{display: 'flex', alignItems: 'center', ...style}}>{children}</div>
);

export const Col: React.FC<{children: React.ReactNode; style?: React.CSSProperties}> = ({children, style}) => (
  <div style={{display: 'flex', flexDirection: 'column', ...style}}>{children}</div>
);

export const Gut: React.FC<{children: React.ReactNode; style?: React.CSSProperties}> = ({children, style}) => {
  const k = useK();
  return <div style={{paddingLeft: GUTTER * k, paddingRight: GUTTER * k, ...style}}>{children}</div>;
};

export const Txt: React.FC<{
  v: string; t: string; color?: string; style?: React.CSSProperties;
}> = ({v, t, color = C.onBg, style}) => {
  const k = useK();
  return <div style={{...type(t, k, color), ...style}}>{v}</div>;
};

/** The status bar — present in every capture, so its absence would read as wrong. */
export const StatusBar: React.FC<{time?: string}> = ({time = '4:23'}) => {
  const k = useK();
  return (
    <Row style={{height: 34 * k, padding: `0 ${14 * k}px`, justifyContent: 'space-between'}}>
      <div style={{...type('labelMedium', k, C.onBg), fontWeight: 600}}>{time}</div>
      <Row style={{gap: 5 * k}}>
        {[0.5, 0.7, 0.9].map((h, i) => (
          <div key={i} style={{width: 3 * k, height: 8 * k * h, background: C.muted, borderRadius: 1 * k}} />
        ))}
        <div
          style={{
            width: 18 * k, height: 9 * k, border: `1.2px solid ${C.muted}`, borderRadius: 2.5 * k,
            marginLeft: 3 * k, position: 'relative',
          }}
        >
          <div style={{position: 'absolute', inset: 1.4 * k, background: C.muted, borderRadius: 1 * k, width: '72%'}} />
        </div>
      </Row>
    </Row>
  );
};

/** Accent-filled CTA (0.9) or the white pill (0.8.9) — the same control, two eras. */
export const Cta: React.FC<{
  label: string; era: 'old' | 'new'; width?: number; accent?: string;
}> = ({label, era, width, accent = C.accent}) => {
  const k = useK();
  const old = era === 'old';
  return (
    <div
      style={{
        width: width ? width * k : undefined,
        height: 56 * k,
        borderRadius: (old ? 28 : 12) * k,
        background: old ? C.onBg : accent,
        color: old ? C.bg : C.onAccent,
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        ...type(old ? 'titleLarge' : 'labelLarge', k),
        fontWeight: 600,
        paddingLeft: old ? 26 * k : 0, paddingRight: old ? 26 * k : 0,
      }}
    >
      {label}
    </div>
  );
};

/** A goal row: glyph chip (0.9 only), name, value, and the meter. */
export const GoalRow: React.FC<{
  name: string; cur: number; target: number; unit?: string;
  fill: number;            // 0..1 — animatable
  accent?: string;         // navy in 0.8.9, red in 0.9
  chip?: boolean;          // the implement chip arrived with 0.9
  glyph?: IconName;        // §12: a goal wears its implement
}> = ({name, cur, target, unit = 'lb', fill, accent = C.accent, chip = true, glyph = 'barbell'}) => {
  const k = useK();
  return (
    <Row style={{gap: 12 * k, marginBottom: 18 * k}}>
      {chip ? (
        <div
          style={{
            width: 44 * k, height: 44 * k, borderRadius: 10 * k, background: C.surfaceVar,
            display: 'flex', alignItems: 'center', justifyContent: 'center', flex: '0 0 auto',
          }}
        >
          <Icon name={glyph} size={22 * k} color={C.muted} />
        </div>
      ) : null}
      <Col style={{flex: 1, gap: 7 * k}}>
        <Row style={{justifyContent: 'space-between', alignItems: 'baseline'}}>
          <div style={type('titleMedium', k, C.onBg)}>{name}</div>
          <div style={type('labelLarge', k, C.muted)}>{`${cur} / ${target} ${unit}`}</div>
        </Row>
        <div style={{height: 10 * k, borderRadius: 999, background: `${C.outline}40`, overflow: 'hidden'}}>
          <div style={{width: `${Math.max(0, Math.min(1, fill)) * 100}%`, height: '100%', background: accent, borderRadius: 999}} />
        </div>
      </Col>
    </Row>
  );
};

/** The seven-day strip. `filled` marks trained days; `now` outlines today. */
export const WeekStrip: React.FC<{filled: number[]; now?: number; accent?: string; era: 'old' | 'new'}> = ({
  filled, now = 0, accent = C.accent, era,
}) => {
  const k = useK();
  const days = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];
  return (
    <Col style={{gap: 9 * k}}>
      <Row style={{justifyContent: 'space-between'}}>
        {days.map((d, i) => (
          <div key={i} style={{...type('labelSmall', k, i === now ? C.onBg : C.muted), width: 38 * k, textAlign: 'center'}}>
            {d}
          </div>
        ))}
      </Row>
      <Row style={{justifyContent: 'space-between'}}>
        {days.map((_, i) => {
          const on = filled.includes(i);
          return (
            <div
              key={i}
              style={{
                width: 38 * k, height: era === 'old' ? 38 * k : 34 * k,
                borderRadius: (era === 'old' ? 9 : 11) * k,
                background: on ? (era === 'old' ? C.onBg : accent) : 'transparent',
                border: on ? 'none' : `1.4px solid ${C.outline}`,
              }}
            />
          );
        })}
      </Row>
    </Col>
  );
};

export const BottomBar: React.FC<{
  items: {label: string; icon: IconName}[]; active: number; accent?: string; badge?: number;
}> = ({items, active, accent = C.accent, badge}) => {
  const k = useK();
  return (
    <Row
      style={{
        position: 'absolute', left: 0, right: 0, bottom: 0, height: 64 * k,
        borderTop: `1px solid ${C.outline}66`, justifyContent: 'space-around', alignItems: 'center',
      }}
    >
      {items.map((it, i) => (
        <Col key={i} style={{alignItems: 'center', gap: 4 * k, position: 'relative'}}>
          <Icon name={it.icon} size={21 * k} color={i === active ? accent : C.muted} />
          <div style={type('labelSmall', k, i === active ? accent : C.muted)}>{it.label}</div>
          {badge && i === items.length - 1 ? (
            <div
              style={{
                position: 'absolute', top: -4 * k, right: -8 * k, background: accent,
                borderRadius: 999, padding: `${1 * k}px ${4 * k}px`, ...type('labelSmall', k, C.onBg),
              }}
            >
              9+
            </div>
          ) : null}
        </Col>
      ))}
    </Row>
  );
};
