/**
 * Pearl (dark default) + the DEFAULT accent, mirrored from forge-android's `Color.kt` / DESIGN.md §5.
 * Onboarding always runs on the default theme (the user hasn't picked an accent yet), so whatever
 * `AccentRed` is set to there is what the cards render around the video — and the warm near-black
 * page is what shows through it. The videos are alpha-transparent: the card's selection wash sits
 * behind them.
 *
 * **These are copies of values, so they go stale silently.** The accent was Navy through the
 * 2026-08-16 warm repaint, and every accent mark on these cards read as a dead pixel until someone
 * noticed. If the default accent or Pearl moves again, change it here AND re-render
 * (`./render.sh all`). The Canvas twin reads the live colour scheme, so it cannot drift this way.
 */
export const ACCENT = '#E23D3D'; // primary: Red, the default accent
export const ON_BG = '#F2EFEA'; // onBackground
export const MUTED = '#BFB6AA'; // onSurfaceVariant
export const OUTLINE = '#38302A'; // outline: empty tracks and cells
export const TILE = '#221C16'; // surfaceVariant: the interactive tile fill

/** `rgba()` of a `#RRGGBB` token. */
export const withAlpha = (hex: string, alpha: number): string => {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
};

/** Straight-line blend of two `#RRGGBB` tokens: Compose's `lerp(Color, Color, t)`. */
export const mix = (from: string, to: string, t: number): string => {
  const a = parseInt(from.slice(1), 16);
  const b = parseInt(to.slice(1), 16);
  const ch = (shift: number) => Math.round(((a >> shift) & 255) + (((b >> shift) & 255) - ((a >> shift) & 255)) * t);
  return `rgb(${ch(16)}, ${ch(8)}, ${ch(0)})`;
};

/** Families registered in `fonts.ts`: stand-ins for Android's sans and monospace. */
export const SANS = 'VignetteSans, sans-serif';
export const MONO = 'VignetteMono, monospace';

/** Shared canvas: the vignette strip inside a plan-mode card, 282×72dp rendered at 4x. */
export const VIGNETTE_WIDTH = 1128;
export const VIGNETTE_HEIGHT = 288;
export const FPS = 30;
/** 1dp on this canvas. Every size below is written as `n * DP` so it can be read against the app. */
export const DP = 4;

/**
 * One seamless pass: frame 0 and the LAST frame are the same finished picture, held still, so the loop
 * restart never jumps. In between it clears and rebuilds. The card plays this twice then holds that
 * last frame — see `PlanModeMedia.kt`. All three compositions share this length and the held-first /
 * held-last shape so the three cards start, loop and freeze together (the Canvas twin's LOOP_MS
 * mirrors it too).
 */
export const LOOP_FRAMES = 150; // 5s @ 30fps

/** Phase timeline, shared so the three cards clear and start rebuilding on the same frames. */
export const HOLD_END = 18; // the opening picture starts to clear
export const CLEAR_END = 32; // …and is gone
export const BUILD_IN = 38; // the rebuild starts
export const BUILD_END = 124; // every card has landed its last mark; the rest is the closing hold

/** 1 while the opening picture is held, 1→0 as it clears, 0 thereafter. */
export const heldOut = (frame: number): number => 1 - smoothstep(frame, HOLD_END, CLEAR_END);

/** 0 before `from`, smoothstepped 0→1 across [from, to], 1 after. */
export function smoothstep(v: number, from: number, to: number): number {
  const t = Math.min(1, Math.max(0, (v - from) / (to - from)));
  return t * t * (3 - 2 * t);
}

/**
 * A critically-damped-looking settle with one small overshoot, 0→1 across `length` frames from
 * `from`. Written out rather than taken from Remotion's `spring()` so the Canvas twin can transcribe
 * it exactly: `1 - e^(-6t)·cos(4.2t)` stays inside 1.07 and is 1 to the eye by t = 1.
 */
export function settle(frame: number, from: number, length: number): number {
  if (frame <= from) return 0;
  const t = Math.min(1, (frame - from) / length);
  return t >= 1 ? 1 : 1 - Math.exp(-6 * t) * Math.cos(4.2 * t);
}

/** A 0→1→0 bump across [from, from + length]: a press, a flash, a bloom. */
export function bump(frame: number, from: number, length: number): number {
  if (frame <= from || frame >= from + length) return 0;
  return Math.sin(((frame - from) / length) * Math.PI);
}

/** Deterministic 0..1 from two integers. The twin carries the same function, so noise matches. */
export function hash01(a: number, b: number): number {
  const x = Math.sin(a * 127.1 + b * 311.7) * 43758.5453;
  return x - Math.floor(x);
}
