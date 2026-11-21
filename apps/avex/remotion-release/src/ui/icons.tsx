import React from 'react';

/**
 * Transcribed, not redrawn. Every path below is the same geometry as the Kotlin `ImageVector` it
 * names, on the same 24-unit viewport — `NavIcons.kt` and `NoticeIcons.kt`. Compose's `curveTo` is
 * a cubic (SVG `C`), `quadTo` a quadratic (`Q`); an even-odd ring (outer circle minus inner) is
 * drawn here as one stroked circle of the same annulus width, which is exactly equivalent.
 */
export type IconName =
  | 'cardio' | 'stats' | 'home' | 'coach' | 'academy' | 'profile' | 'bell' | 'gear'
  | 'barbell' | 'machine';

/** ExerciseIcons.roundRect(l, t, r, b, radius) → an SVG rect. */
const RR: React.FC<{l: number; t: number; r: number; b: number; rad: number; fill?: string; stroke?: string; sw?: number}> = ({
  l, t, r, b, rad, fill = 'none', stroke, sw,
}) => (
  <rect x={l} y={t} width={r - l} height={b - t} rx={rad} ry={rad} fill={fill} stroke={stroke} strokeWidth={sw} />
);

const bar = (cx: number, top: number) => {
  const hw = 1.6, bottom = 20, r = 1, l = cx - hw, right = cx + hw;
  return `M${l},${bottom} L${l},${top + r} Q${l},${top} ${l + r},${top} L${right - r},${top} Q${right},${top} ${right},${top + r} L${right},${bottom} Z`;
};

export const Icon: React.FC<{name: IconName; size: number; color: string; opacity?: number}> = ({
  name, size, color, opacity = 1,
}) => {
  const p = {fill: color, stroke: 'none'};
  const s = {fill: 'none', stroke: color, strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const};
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" style={{opacity, display: 'block', flex: '0 0 auto'}}>
      {name === 'cardio' && (
        <path {...s} strokeWidth={2.2} d="M2.5,12 L8.5,12 L11,6.5 L13,17.5 L15.5,12 L21.5,12" />
      )}
      {name === 'stats' && (
        <path {...p} d={`${bar(5.4, 12.5)} ${bar(12, 8.5)} ${bar(18.6, 4.5)}`} />
      )}
      {name === 'home' && (
        <path {...p} fillRule="evenodd" d="M12,2.8 L21,10.8 L21,21 L3,21 L3,10.8 Z M9.8,14.2 L14.2,14.2 L14.2,21 L9.8,21 Z" />
      )}
      {name === 'coach' && (
        <>
          {/* the even-odd ring: r 9 outer, 7.3 inner → one stroke of width 1.7 at r 8.15 */}
          <circle cx={12} cy={12} r={8.15} fill="none" stroke={color} strokeWidth={1.7} />
          <path {...p} d="M13.36,13.36 L8.28,15.72 L10.64,10.64 L15.72,8.28 Z" />
        </>
      )}
      {name === 'academy' && (
        <>
          <path
            {...s} strokeWidth={1.9}
            d="M12,7.2 C10.2,5.4 7.4,4.6 3.4,4.8 L3.4,17.6 C7.4,17.4 10.2,18.2 12,20 C13.8,18.2 16.6,17.4 20.6,17.6 L20.6,4.8 C16.6,4.6 13.8,5.4 12,7.2 Z"
          />
          <path {...s} strokeWidth={1.9} d="M12,7.2 L12,20" />
        </>
      )}
      {name === 'profile' && (
        <>
          <circle cx={12} cy={7.6} r={3.6} fill={color} />
          <path
            {...p}
            d="M5.9,18.4 C5.9,14.6 8.6,12.8 12,12.8 C15.4,12.8 18.1,14.6 18.1,18.4 L18.1,18.7 Q18.1,20.1 16.7,20.1 L7.3,20.1 Q5.9,20.1 5.9,18.7 Z"
          />
        </>
      )}
      {name === 'bell' && (
        <>
          <path {...s} strokeWidth={1.9} d="M5.4,17.2 L5.4,10.8 A6.6,6.6 0 1 1 18.6,10.8 L18.6,17.2 Z" />
          <path {...s} strokeWidth={1.9} d="M10.2,20 L13.8,20" />
        </>
      )}
      {name === 'gear' && (
        <>
          {/* a toothed ring, not a starburst: teeth are stubby rounded rects on the rim */}
          {Array.from({length: 8}).map((_, i) => (
            <rect
              key={i} x={10.75} y={1.9} width={2.5} height={4.4} rx={1} fill={color}
              transform={`rotate(${i * 45} 12 12)`}
            />
          ))}
          <circle cx={12} cy={12} r={6.4} fill="none" stroke={color} strokeWidth={2.6} />
          <circle cx={12} cy={12} r={2.5} fill={color} />
        </>
      )}
      {name === 'barbell' && (
        <>
          <path {...s} strokeWidth={1.8} d="M2.8,12 L21.2,12" />
          <RR l={5.6} t={7.6} r={8.0} b={16.4} rad={1} fill={color} />
          <RR l={16.0} t={7.6} r={18.4} b={16.4} rad={1} fill={color} />
        </>
      )}
      {name === 'machine' && (
        <>
          <RR l={6.8} t={7.2} r={17.2} b={19} rad={1.5} stroke={color} sw={1.7} />
          <path {...s} strokeWidth={1.7} d="M6.8,11.2 L17.2,11.2 M6.8,15.1 L17.2,15.1" />
          <path {...s} strokeWidth={1.7} d="M12,7.2 L12,3.8" />
        </>
      )}
    </svg>
  );
};
