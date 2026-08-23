/**
 * Pearl (dark default) + the DEFAULT accent, mirrored from forge-android's `Color.kt` / DESIGN.md §5.
 * Onboarding always runs on the default theme (the user hasn't picked an accent yet), so whatever
 * `AccentRed` is set to there is what the cards render around the video — and the warm near-black
 * page is what shows through it. The videos are alpha-transparent: the card's selection wash sits
 * behind them.
 *
 * **This is a copy of a value, so it goes stale silently.** It was Navy through the 2026-08-16 warm
 * repaint, and every accent mark on these cards read as a dead pixel until someone noticed. If the
 * default accent moves again, change it here AND re-render (`./render.sh all`).
 */
export const ACCENT = '#E23D3D'; // Red — accent 1.0: the work you're shown (§5 "bars fill primary")
export const ACCENT_GLOW = 'rgba(226, 61, 61, 0.45)'; // the bloom under a mark as it lands
export const MUTED = '#BFB6AA'; // mono day labels

/** Shared canvas: the vignette strip inside a plan-mode card, 282×72dp rendered at 4x. */
export const VIGNETTE_WIDTH = 1128;
export const VIGNETTE_HEIGHT = 288;
export const FPS = 30;

/**
 * One seamless pass: frame 0 and the LAST frame are the same finished plan, held still, so the loop
 * restart never jumps. In between the plan clears and rebuilds. The card plays this twice then holds
 * that last frame — see `PlanModeMedia.kt`. All three compositions share this length and the
 * held-first / held-last shape so the three cards start, loop and freeze together (the Canvas
 * fallbacks' LOOP_MS mirrors it too).
 */
export const LOOP_FRAMES = 150; // 5s @ 30fps

/** Phase timeline, shared so the three cards clear and start rebuilding on the same frames. */
export const HOLD_END = 18; // the opening plan starts to clear
export const CLEAR_END = 32; // …and is gone
export const BUILD_IN = 38; // the rebuild starts
export const BUILD_END = 124; // …the generated sweep leaves the strip; the hand-placed cards land
//                              their last mark just before it and settle into the closing hold

/** 1 while the opening plan is held, 1→0 as it clears, 0 thereafter. */
export const heldOut = (frame: number): number =>
  1 - smoothstep(frame, HOLD_END, CLEAR_END);

/** 0 before `from`, smoothstepped 0→1 across [from, to], 1 after. */
export function smoothstep(v: number, from: number, to: number): number {
  const t = Math.min(1, Math.max(0, (v - from) / (to - from)));
  return t * t * (3 - 2 * t);
}
