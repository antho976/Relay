/**
 * Tokens lifted verbatim from forge-android's Color.kt so the film is the product's own palette,
 * not an approximation of it. AccentRed became the default accent on 2026-08-23 (Navy → Ember → Red),
 * which is exactly why the 0.8.9 footage reads navy and the 0.9 footage reads red.
 */
export const BG        = '#110F0C';   // PearlBackground
export const SURFACE   = '#1A1613';   // PearlSurface
export const SURFACE_V = '#221C16';   // PearlSurfaceVar
export const OUTLINE   = '#38302A';   // PearlOutline
export const ON_BG     = '#F2EFEA';   // PearlOnBg
export const MUTED     = '#BFB6AA';   // PearlMuted
export const ACCENT    = '#E23D3D';   // AccentRed — 0.9 default
export const ACCENT_OLD= '#3D4F73';   // AccentNavy — what 0.8.9 shipped with

export const W = 1920;
export const H = 1080;
export const FPS = 30;

/** Device captures are 1080×2400. */
export const SHOT_W = 1080;
export const SHOT_H = 2400;
export const SHOT_AR = SHOT_H / SHOT_W;

export const SERIF = 'Georgia, "Times New Roman", ui-serif, serif';
export const MONO  = '"JetBrains Mono", "DejaVu Sans Mono", ui-monospace, monospace';
export const SANS  = 'Inter, "Helvetica Neue", Arial, ui-sans-serif, sans-serif';

/**
 * One clock for the whole film. The 0.8.9 cut ran 3:36 because every beat was ~11s and entered the
 * same way; at 90s the edit has to carry three speeds, and a beat's speed decides how much copy it
 * is allowed to hold. HERO gets a paragraph, BEAT gets one line, FLASH gets a headline only.
 */
export const SEC = (n: number) => Math.round(n * FPS);

export const PACE = {
  hero:  SEC(6.5),
  beat:  SEC(4.5),
  flash: SEC(3),
  card:  SEC(4),
} as const;

/** Transition lengths. A hard cut is genuinely zero — that is what makes a short edit feel fast. */
export const XFADE = {cut: 0, quick: 7, soft: 12, pan: 14, wide: 16} as const;
