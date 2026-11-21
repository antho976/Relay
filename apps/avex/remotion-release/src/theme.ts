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

/**
 * The bed's grid, MEASURED from `public/music/bed.mp3` rather than assumed from the prompt.
 *
 * Method: spectral-flux onset envelope at a 1.45 ms hop → the onset peak nearest each predicted
 * beat → a least-squares line through 110 of them. Period 0.49991 s (120.021 BPM), first beat at
 * 0.0082 s, residual 2.2 ms rms / 9.6 ms max. Downbeat = the phase carrying the most low-band flux.
 *
 * The previous figure (120.19 BPM, first downbeat 0.116 s) came from an autocorrelation with a
 * coarser hop and put every cut in the first half of the film 2-3 frames LATE; the two grids only
 * agreed around bar 33. At 30 fps the corrected bar is 59.99 frames, so bar n sits at frame
 * 60·(n−1) to within rounding and the film is exactly 2700 frames.
 */
export const BED = {firstDownbeat: 0.0082, bar: 1.99964, bars: 46} as const;

/** Frame of the downbeat that opens bar `n` (1-indexed). */
export const bar = (n: number) => Math.round((BED.firstDownbeat + (n - 1) * BED.bar) * FPS);

/* ── the sound grid ──────────────────────────────────────────────────────── */

/**
 * Cues used to fire wherever their picture happened to be, which against a 120 BPM bed is the
 * difference between a sound design and a pile of noises. Everything audible now lands on a
 * subdivision of the same grid the cuts use.
 *
 * At this tempo a bar is 59.99 frames, a quarter 15.00, an eighth 7.50 and a sixteenth 3.75. None of
 * those are exactly whole frames, so a run of cues has to be laid out in float and rounded per event —
 * rounding the step first would drift a sixteenth of a beat every four events.
 */
export const BAR_F = BED.bar * FPS;
export const QUARTER = BAR_F / 4;
export const EIGHTH = BAR_F / 8;
export const SIXTEENTH = BAR_F / 16;

const FIRST_F = BED.firstDownbeat * FPS;

/** Snap an absolute frame to the nearest grid point. `div` is subdivisions per bar. */
export const snap = (f: number, div = 8) => {
  const step = BAR_F / div;
  return Math.round(FIRST_F + Math.round((f - FIRST_F) / step) * step);
};

/**
 * A run of `count` events at `step` frames apart, beginning at the grid point at or after `from`.
 * Returned as absolute frames. Used for the onboarding pages landing, the watch stepper and the
 * list settling, all of which were previously spaced by whatever looked right.
 */
export const run = (from: number, count: number, step: number, div = 16): number[] => {
  const first = snap(from, div);
  return Array.from({length: count}, (_, i) => Math.round(first + i * step));
};
