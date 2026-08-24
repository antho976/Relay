import React from 'react';
import {interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {ACCENT, MONO, MUTED, ON_BG, SERIF} from './theme';

/** Rise-and-fade used by every text block, so nothing enters a different way to anything else. */
export const useRise = (delay = 0) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const s = spring({frame: frame - delay, fps, config: {damping: 200, stiffness: 90}});
  return {opacity: s, transform: `translateY(${(1 - s) * 18}px)`};
};

export const Eyebrow: React.FC<{children: React.ReactNode; delay?: number}> = ({children, delay = 0}) => (
  <div
    style={{
      fontFamily: MONO, fontSize: 22, letterSpacing: 4, color: ACCENT,
      textTransform: 'uppercase', ...useRise(delay),
    }}
  >
    {children}
  </div>
);

export const Title: React.FC<{children: React.ReactNode; delay?: number; size?: number}> = ({
  children, delay = 0, size = 76,
}) => (
  <div style={{fontFamily: SERIF, fontSize: size, lineHeight: 1.06, color: ON_BG, ...useRise(delay)}}>
    {children}
  </div>
);

export const Body: React.FC<{children: React.ReactNode; delay?: number}> = ({children, delay = 0}) => (
  <div
    style={{
      fontFamily: SERIF, fontSize: 30, lineHeight: 1.45, color: MUTED,
      maxWidth: 620, fontStyle: 'italic', ...useRise(delay),
    }}
  >
    {children}
  </div>
);

/** The 0.8.9 / 0.9 tag that sits under each phone in a comparison. */
export const VersionTag: React.FC<{label: string; accent?: boolean; delay?: number}> = ({
  label, accent = false, delay = 0,
}) => {
  const r = useRise(delay);
  return (
    <div
      style={{
        fontFamily: MONO, fontSize: 20, letterSpacing: 3, textTransform: 'uppercase',
        color: accent ? ACCENT : MUTED,
        border: `1.5px solid ${accent ? ACCENT : '#38302A'}`,
        borderRadius: 999, padding: '8px 22px', ...r,
      }}
    >
      {label}
    </div>
  );
};

/** A soft vignette so the dark plate never reads as flat black. */
export const Plate: React.FC<{children: React.ReactNode}> = ({children}) => (
  <div
    style={{
      position: 'absolute', inset: 0,
      background: 'radial-gradient(120% 90% at 50% 18%, #17120E 0%, #110F0C 55%, #0A0806 100%)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
    }}
  >
    {children}
  </div>
);

/** Frame-accurate fade at the head and tail of a scene. */
export const useEdgeFade = (durationInFrames: number, len = 12) => {
  const frame = useCurrentFrame();
  return interpolate(
    frame,
    [0, len, durationInFrames - len, durationInFrames],
    [0, 1, 1, 0],
    {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'}
  );
};
