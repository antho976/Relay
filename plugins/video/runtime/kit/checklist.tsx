// A list that ticks in: items appearing one after another, each with a check that draws itself, a
// counter and a progress bar — "everything else in this release" on the beat. `mode: 'one'` is
// the montage: one item at a time, large, replacing the last.
import React from 'react';
import { interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';
import { easeOut } from './motion';
import { theme } from './theme';

const clamp = (v: number) => Math.max(0, Math.min(1, v));

export const Checklist: React.FC<{
  items: string[];
  /** The frame the first item ticks in (in the sequence it is in). */
  start?: number;
  /** Frames between items: `grid.framesPerBeat` to tick on every beat, half that for every
   * eighth. */
  every?: number;
  /** `list` (default): the items stack up. `one`: a montage, one item at a time. */
  mode?: 'list' | 'one';
  title?: string;
  /** The `3/12` counter (default true). */
  counter?: boolean;
  /** The progress bar (default true). */
  progress?: boolean;
  /** The upcoming items, dimmed, before they tick (list mode). */
  ghosts?: boolean;
  size?: number;
  width?: number;
  color?: string;
  accent?: string;
  muted?: string;
  font?: string;
  id?: string;
}> = ({
  items, start = 0, every = 15, mode = 'list', title, counter = true, progress = true, ghosts = false, size = 44, width = 880,
  color = theme.colors.text, accent = theme.colors.accent, muted = theme.colors.muted, font = theme.fonts.body, id = 'checklist',
}) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const at = (i: number) => start + i * every;
  const ticked = items.filter((_, i) => frame >= at(i)).length;
  // The bar and counter follow the ticks on a spring, so they settle rather than step.
  const shown = items.reduce((sum, _, i) => sum + spring({ frame: frame - at(i), fps, config: { damping: 200 }, durationInFrames: Math.max(4, Math.min(every, 12)) }), 0);
  const fill = items.length ? clamp(shown / items.length) : 0;
  const check = (p: number, s: number) => (
    <svg width={s} height={s} viewBox="0 0 24 24" style={{ flex: 'none' }}>
      <circle cx="12" cy="12" r="11" fill={p > 0 ? accent : 'none'} stroke={p > 0 ? accent : muted} strokeOpacity={p > 0 ? 1 : 0.5} strokeWidth="1.5"
        style={{ transform: `scale(${0.7 + 0.3 * Math.min(1, p * 1.4)})`, transformOrigin: '12px 12px' }} />
      <path d="M7 12.5l3.2 3.2L17 9" fill="none" stroke={theme.colors.accentText} strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round"
        strokeDasharray="16" strokeDashoffset={16 * (1 - clamp(p * 1.6 - 0.3))} />
    </svg>
  );
  const head = (
    <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', gap: size * 0.5 }}>
      {title ? <div data-id={`${id}-title`} style={{ fontFamily: font, fontWeight: 700, fontSize: size * 1.1, color }}>{title}</div> : <div />}
      {counter ? (
        <div data-id={`${id}-counter`} style={{ fontFamily: theme.fonts.mono, fontSize: size * 0.8, color: accent, fontVariantNumeric: 'tabular-nums' }}>
          {ticked}/{items.length}
        </div>
      ) : null}
    </div>
  );
  const bar = progress ? (
    <div style={{ height: Math.max(6, size * 0.16), borderRadius: size, background: `${muted}40`, overflow: 'hidden' }}>
      <div data-id={`${id}-progress`} style={{ width: `${fill * 100}%`, height: '100%', borderRadius: size, background: accent }} />
    </div>
  ) : null;

  if (mode === 'one') {
    const current = Math.max(0, ticked - 1);
    const p = interpolate(frame - at(current), [0, Math.min(10, every)], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeOut });
    return (
      <div data-id={id} style={{ width, display: 'flex', flexDirection: 'column', gap: size * 0.6 }}>
        {head}
        {bar}
        <div style={{ display: 'flex', alignItems: 'center', gap: size * 0.5, minHeight: size * 2.2 }}>
          {ticked > 0 ? check(p, size * 1.6) : null}
          {ticked > 0 ? (
            <div data-id={`${id}-item`} style={{ fontFamily: font, fontWeight: 700, fontSize: size * 1.4, color, opacity: p, transform: `translateY(${(1 - p) * size * 0.4}px)` }}>
              {items[current]}
            </div>
          ) : null}
        </div>
      </div>
    );
  }

  return (
    <div data-id={id} style={{ width, display: 'flex', flexDirection: 'column', gap: size * 0.45 }}>
      {head}
      {bar}
      <div style={{ display: 'flex', flexDirection: 'column', gap: size * 0.35, marginTop: size * 0.2 }}>
        {items.map((item, i) => {
          const p = interpolate(frame - at(i), [0, Math.min(10, every)], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeOut });
          if (p <= 0 && !ghosts) return null;
          return (
            <div key={i} data-id={`${id}-item-${i}`} style={{
              display: 'flex', alignItems: 'center', gap: size * 0.4, opacity: p > 0 ? 0.3 + 0.7 * p : 0.3,
              transform: `translateX(${(1 - p) * size * 0.6}px)`,
            }}>
              {check(p, size)}
              <div style={{ fontFamily: font, fontSize: size, color: p > 0 ? color : muted }}>{item}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
