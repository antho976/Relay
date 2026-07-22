/**
 * Pearl (dark default) + Navy accent tokens from forge-android's Color.kt / DESIGN.md §5.
 * Onboarding always runs on the default theme (the user hasn't picked an accent yet), so the
 * baked-in navy matches what the tiles render behind the video. Videos are alpha-transparent —
 * the card's selection wash shows through.
 */
export const ACCENT = '#3D4F73';           // Navy, accent 1.0 (bars, live strokes)
export const ACCENT_A60 = 'rgba(61, 79, 115, 0.6)';
export const OUTLINE_A35 = 'rgba(46, 46, 56, 0.35)';  // #2E2E38 @ 0.35 — frames/borders
export const MUTED = '#B4B4C2';            // secondary marks
export const ON_BG = '#EEEEF2';

/** Shared canvas: the vignette strip inside a plan-mode card, 282×72dp rendered at 4x. */
export const VIGNETTE_WIDTH = 1128;
export const VIGNETTE_HEIGHT = 288;
export const FPS = 30;
/** One seamless pass: frame 0 and the LAST frame are the same finished plan, held still, so the loop
 *  restart never jumps; in between the plan clears and rebuilds. The card loops this twice then holds
 *  the last frame (the plan). Both videos share this length + held-first / held-last shape so the two
 *  cards start, loop, and freeze together (the Canvas fallbacks' LOOP_MS mirrors it too). */
export const LOOP_FRAMES = 240;
