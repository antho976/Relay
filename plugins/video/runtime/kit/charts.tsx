// Charts that draw themselves in: a donut (or pie) and bars, the shapes an app promo or a stats
// video shows most. Plain SVG, each a function of the frame like everything else.
import React from 'react';
import { interpolate, useCurrentFrame } from 'remotion';
import { easeOut, stagger, useProgress, useSpring } from './motion';
import { theme } from './theme';

export type Segment = { value: number; color: string; label?: string };

/** A donut whose segments sweep in over `duration` frames after `delay`, `size` px across, its
 * ring `thickness` px (half the size makes a pie). `highlight` lifts one segment (by index) out a
 * little; the children sit in its middle (a total, a label). */
export const Donut: React.FC<{
  id?: string; segments: Segment[]; size?: number; thickness?: number; gap?: number; track?: string;
  delay?: number; duration?: number; highlight?: number; children?: React.ReactNode; style?: React.CSSProperties;
}> = ({ id, segments, size = 360, thickness = 44, gap = 2, track = 'rgba(255,255,255,0.08)', delay = 0, duration = 36, highlight, children, style }) => {
  const p = useProgress({ delay, duration });
  const lift = useSpring(delay + duration);
  const total = segments.reduce((sum, s) => sum + Math.max(0, s.value), 0) || 1;
  const r = (size - thickness) / 2 - 6;
  const length = 2 * Math.PI * r;
  let start = 0;
  return (
    <div data-id={id} style={{ position: 'relative', width: size, height: size, ...style }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} style={{ transform: 'rotate(-90deg)' }}>
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={track} strokeWidth={thickness} />
        {segments.map((s, i) => {
          const share = Math.max(0, s.value) / total;
          const from = start;
          start += share;
          // Each segment draws in the part of the sweep it covers.
          const shown = interpolate(p, [from, from + share], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
          const arc = Math.max(0, share * length * shown - (shown > 0 ? gap : 0));
          const raised = highlight === i ? lift * 8 : 0;
          const mid = (from + share / 2) * 2 * Math.PI;
          return (
            <circle key={i} cx={size / 2 + Math.cos(mid) * raised} cy={size / 2 + Math.sin(mid) * raised} r={r} fill="none"
              stroke={s.color} strokeWidth={thickness + (highlight === i ? lift * 6 : 0)}
              strokeDasharray={`${arc} ${length}`} strokeDashoffset={-from * length} />
          );
        })}
      </svg>
      <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', flexDirection: 'column' }}>
        {children}
      </div>
    </div>
  );
};

/** Bars growing from their base one after another, `stagger` frames apart: `values` with
 * optional `labels` under them and the values over them (`format` writes one). `colors` is one
 * colour or one per bar; `highlight` draws one bar (by index) in the accent and the rest dimmed. */
export const Bars: React.FC<{
  id?: string; values: number[]; labels?: string[]; colors?: string | string[]; max?: number; width?: number; height?: number;
  gap?: number; radius?: number; delay?: number; step?: number; highlight?: number; showValues?: boolean;
  format?: (value: number) => string; font?: string; labelColor?: string; style?: React.CSSProperties;
}> = ({
  id, values, labels, colors = theme.colors.accent, max, width = 720, height = 420, gap = 18, radius = 10, delay = 0, step = 4, highlight,
  showValues = false, format = (v) => String(Math.round(v)), font = theme.fonts.body, labelColor = theme.colors.muted, style,
}) => {
  const frame = useCurrentFrame();
  const top = max ?? Math.max(1, ...values);
  const bar = (width - gap * (values.length - 1)) / Math.max(1, values.length);
  const labelSize = Math.max(18, Math.min(32, bar * 0.32));
  const chart = height - (labels ? labelSize * 1.8 : 0) - (showValues ? labelSize * 1.6 : 0);
  return (
    <div data-id={id} style={{ width, height, display: 'flex', alignItems: 'flex-end', gap, ...style }}>
      {values.map((v, i) => {
        const p = interpolate(frame - stagger(i, step, delay), [0, 24], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeOut });
        const colour = Array.isArray(colors) ? colors[i % colors.length] : colors;
        const dimmed = highlight !== undefined && highlight !== i;
        return (
          <div key={i} style={{ width: bar, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: labelSize * 0.4 }}>
            {showValues ? <div style={{ fontFamily: font, fontSize: labelSize, fontWeight: 700, color: theme.colors.text, opacity: p }}>{format(v * p)}</div> : null}
            <div style={{ width: '100%', height: Math.max(0, (v / top) * chart * p), borderRadius: radius, background: colour, opacity: dimmed ? 0.35 : 1 }} />
            {labels ? <div style={{ fontFamily: font, fontSize: labelSize, color: labelColor }}>{labels[i]}</div> : null}
          </div>
        );
      })}
    </div>
  );
};
