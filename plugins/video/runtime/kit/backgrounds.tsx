// Grounds that move a little, so a still scene does not look frozen: a turning gradient, a
// grid, drifting glows and film grain. Each fills its parent (put it first in an AbsoluteFill).
import { noise2D } from '@remotion/noise';
import React from 'react';
import { AbsoluteFill, useCurrentFrame, useVideoConfig } from 'remotion';
import { theme } from './theme';

/** A linear gradient that turns `turn` degrees over the composition (default 30). */
export const Gradient: React.FC<{ from?: string; to?: string; angle?: number; turn?: number }> = ({
  from = theme.colors.background, to = theme.colors.surface, angle = 160, turn = 30,
}) => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();
  const a = angle + (turn * frame) / Math.max(1, durationInFrames);
  return <AbsoluteFill style={{ background: `linear-gradient(${a}deg, ${from}, ${to})` }} />;
};

/** Grid lines `size` px apart, fading out from the middle. */
export const Grid: React.FC<{ color?: string; size?: number; opacity?: number; drift?: number }> = ({
  color = '#FFFFFF', size = 72, opacity = 0.07, drift = 0.3,
}) => {
  const frame = useCurrentFrame();
  const shift = (frame * drift) % size;
  const line = `${color} 0 1px, transparent 1px ${size}px`;
  return (
    <AbsoluteFill style={{
      opacity,
      backgroundImage: `repeating-linear-gradient(0deg, ${line}), repeating-linear-gradient(90deg, ${line})`,
      backgroundPosition: `${shift}px ${shift}px`,
      maskImage: 'radial-gradient(ellipse at center, black 30%, transparent 75%)',
      WebkitMaskImage: 'radial-gradient(ellipse at center, black 30%, transparent 75%)',
    }} />
  );
};

/** A soft coloured light around (`x`, `y`) — fractions of the frame — that drifts slowly. */
export const Glow: React.FC<{ x?: number; y?: number; size?: number; color?: string; opacity?: number; seed?: string }> = ({
  x = 0.5, y = 0.35, size = 0.9, color = theme.colors.accent, opacity = 0.35, seed = 'glow',
}) => {
  const frame = useCurrentFrame();
  const { width, height } = useVideoConfig();
  const dx = noise2D(seed, frame * 0.004, 0) * 0.08;
  const dy = noise2D(seed, 0, frame * 0.004) * 0.08;
  const d = Math.max(width, height) * size;
  return (
    <AbsoluteFill style={{ overflow: 'hidden', pointerEvents: 'none' }}>
      <div style={{
        position: 'absolute', left: (x + dx) * width - d / 2, top: (y + dy) * height - d / 2, width: d, height: d,
        borderRadius: '50%', opacity, background: `radial-gradient(circle, ${color} 0%, transparent 62%)`,
      }} />
    </AbsoluteFill>
  );
};

/** Film grain over everything, new every frame. */
export const Grain: React.FC<{ opacity?: number }> = ({ opacity = 0.06 }) => {
  const frame = useCurrentFrame();
  return (
    <AbsoluteFill style={{ opacity, mixBlendMode: 'overlay', pointerEvents: 'none' }}>
      <svg width="100%" height="100%">
        <filter id={`grain-${frame}`}>
          <feTurbulence type="fractalNoise" baseFrequency="0.85" numOctaves={2} seed={frame % 97} stitchTiles="stitch" />
        </filter>
        <rect width="100%" height="100%" filter={`url(#grain-${frame})`} />
      </svg>
    </AbsoluteFill>
  );
};
