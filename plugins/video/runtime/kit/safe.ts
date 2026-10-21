// The part of a frame text should stay inside: free of the buttons, names and captions a
// platform lays over a vertical video, or a TV's title-safe margin on a wide one. video.still
// flags text outside it as `unsafe`, by these same numbers (crates/relay-core/src/video/probe.rs).
import type React from 'react';
import { useVideoConfig } from 'remotion';

export type Insets = { top: number; right: number; bottom: number; left: number };

/** Insets in px for a frame `width` × `height`. A frame taller than 3:2 is a phone's
 * (Shorts, Reels, TikTok): 10% top, 12% right, 20% bottom, 6% left. Any other keeps 5% each side. */
export const safeArea = (width: number, height: number): Insets => {
  if (height > width * 1.5) {
    return { top: height * 0.1, right: width * 0.12, bottom: height * 0.2, left: width * 0.06 };
  }
  return { top: height * 0.05, right: width * 0.05, bottom: height * 0.05, left: width * 0.05 };
};

/** The composition's safe area. */
export const useSafeArea = (): Insets => {
  const { width, height } = useVideoConfig();
  return safeArea(width, height);
};

/** Padding that keeps an AbsoluteFill's content inside the safe area. */
export const useSafePadding = (): React.CSSProperties => {
  const s = useSafeArea();
  return { paddingTop: s.top, paddingRight: s.right, paddingBottom: s.bottom, paddingLeft: s.left };
};
