// A starting look: one ground, one accent, a type ramp sized for video (read from across a room
// or on a phone at arm's length, not on a desk). A video's own src/theme.ts usually replaces it.

export type Theme = {
  colors: {
    background: string;
    surface: string;
    text: string;
    muted: string;
    accent: string;
    accentText: string;
  };
  fonts: { display: string; body: string; mono: string };
  /** Font sizes in px for a 1080 px wide frame; `scaleType` fits them to another width. */
  type: { display: number; title: number; subtitle: number; body: number; caption: number };
  radius: number;
};

export const theme: Theme = {
  colors: {
    background: '#0B0D12',
    surface: '#161A23',
    text: '#F5F7FA',
    muted: '#9AA3B2',
    accent: '#7DD3FC',
    accentText: '#06202B',
  },
  // The kit's own families (fonts.ts), there on every machine.
  fonts: {
    display: 'Inter, sans-serif',
    body: 'Inter, sans-serif',
    mono: '"JetBrains Mono", monospace',
  },
  type: { display: 128, title: 88, subtitle: 52, body: 40, caption: 34 },
  radius: 28,
};

/** A theme with some of its parts replaced. */
export const defineTheme = (parts: Partial<{ [K in keyof Theme]: Partial<Theme[K]> }>): Theme => ({
  colors: { ...theme.colors, ...parts.colors },
  fonts: { ...theme.fonts, ...parts.fonts },
  type: { ...theme.type, ...parts.type },
  radius: parts.radius ?? theme.radius,
} as Theme);

/** A type size from the ramp (made for 1080 px wide) fitted to a frame `width` px wide. */
export const scaleType = (size: number, width: number): number => Math.round((size * width) / 1080);
