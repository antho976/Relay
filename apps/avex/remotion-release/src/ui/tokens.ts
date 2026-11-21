/**
 * The app's design system, transcribed for the film. Every value here is copied from
 * forge-android — Color.kt for the palette, Type.kt for the scale — so a recreated screen is the
 * product's own geometry rather than an impression of it.
 *
 * Written in **dp**. A screen renders at any pixel size via `scale`; the device captures were
 * 1080×2400 at 450dpi, i.e. 2.8125 px/dp → a 384×853dp screen.
 */
export const SCREEN_DP = {w: 384, h: 853};

export const C = {
  bg: '#110F0C',
  surface: '#1A1613',
  surfaceVar: '#221C16',
  outline: '#38302A',
  onBg: '#F2EFEA',
  muted: '#BFB6AA',
  accent: '#E23D3D',      // 0.9 default
  accentOld: '#3D4F73',   // what 0.8.9 shipped with
  onAccent: '#110F0C',
};

const SERIF = 'Georgia, "Times New Roman", ui-serif, serif';
const SANS = 'Inter, "Helvetica Neue", Arial, ui-sans-serif, sans-serif';
const MONO = '"JetBrains Mono", "DejaVu Sans Mono", ui-monospace, monospace';

type Style = {f: string; s: number; lh: number; ls?: number; w?: number};

/** MaterialTheme.typography, one for one. */
export const T: Record<string, Style> = {
  displayLarge:   {f: SERIF, s: 52, lh: 58, ls: 0},
  headlineLarge:  {f: SERIF, s: 36, lh: 42, ls: 0},
  headlineMedium: {f: SERIF, s: 28, lh: 34, ls: 0},
  headlineSmall:  {f: SERIF, s: 22, lh: 28, ls: 0},
  titleLarge:     {f: SANS,  s: 18, lh: 24, ls: 0,    w: 500},
  titleMedium:    {f: SANS,  s: 16, lh: 22, ls: 0.15, w: 500},
  titleSmall:     {f: SANS,  s: 14, lh: 20, ls: 0.1,  w: 500},
  bodyLarge:      {f: SANS,  s: 16, lh: 24, ls: 0},
  bodyMedium:     {f: SANS,  s: 14, lh: 20, ls: 0},
  bodySmall:      {f: SANS,  s: 12, lh: 16, ls: 0},
  labelLarge:     {f: MONO,  s: 13, lh: 16, ls: 0.8,  w: 500},
  labelMedium:    {f: MONO,  s: 11, lh: 14, ls: 0.6},
  labelSmall:     {f: MONO,  s: 10, lh: 12, ls: 0.5},
};

/** Turn a token into CSS at a given dp→px scale. */
export const type = (name: keyof typeof T | string, k: number, color?: string): React.CSSProperties => {
  const t = T[name as string];
  return {
    fontFamily: t.f,
    fontSize: t.s * k,
    lineHeight: `${t.lh * k}px`,
    letterSpacing: (t.ls ?? 0) * k,
    fontWeight: t.w ?? 400,
    color,
    margin: 0,
  };
};

/** §1: the page gutter is 24dp everywhere. */
export const GUTTER = 24;
