// Captions a word at a time, as short-video viewers expect: a few words on screen, the one being
// said lit. From timed words (a transcript with word timings, or an .srt through
// @remotion/captions' parseSrt), or from plain text paced at a speaking rate.
import { createTikTokStyleCaptions, type Caption } from '@remotion/captions';
import React, { useMemo } from 'react';
import { useCurrentFrame, useVideoConfig } from 'remotion';
import { useSafeArea } from './safe';
import { theme } from './theme';

export type { Caption };

/** Timed words from plain `text`, one after another from `startMs` at `wordsPerMinute` (default
 * 160, an easy speaking pace): captions for a video with no voice to time them by. */
export const captionsFromText = (text: string, startMs = 0, wordsPerMinute = 160): Caption[] => {
  const per = 60000 / wordsPerMinute;
  return text.split(/\s+/).filter(Boolean).map((word, i) => ({
    text: (i === 0 ? '' : ' ') + word,
    startMs: startMs + i * per,
    endMs: startMs + (i + 1) * per,
    timestampMs: startMs + i * per + per / 2,
    confidence: null,
  }));
};

/** Caption pages along the bottom of the safe area (or its middle or top), the word being said
 * in `highlight`. */
export const Captions: React.FC<{
  captions: Caption[];
  /** Its data-id. */
  id?: string;
  /** Words this close together share a page; default 1200 ms. */
  combineWithinMs?: number;
  position?: 'bottom' | 'center' | 'top';
  size?: number;
  color?: string;
  highlight?: string;
  font?: string;
  weight?: number;
  style?: React.CSSProperties;
}> = ({
  captions, id, combineWithinMs = 1200, position = 'bottom', size = 64, color = '#FFFFFF', highlight = theme.colors.accent,
  font = theme.fonts.display, weight = 800, style,
}) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const safe = useSafeArea();
  const { pages } = useMemo(() => createTikTokStyleCaptions({ captions, combineTokensWithinMilliseconds: combineWithinMs }), [captions, combineWithinMs]);
  const ms = (frame / fps) * 1000;
  const page = pages.find((p) => ms >= p.startMs && ms < p.startMs + p.durationMs);
  if (!page) return null;
  const place: React.CSSProperties = position === 'bottom'
    ? { bottom: safe.bottom, justifyContent: 'flex-end' }
    : position === 'top' ? { top: safe.top } : { top: 0, bottom: 0, justifyContent: 'center' };
  return (
    <div style={{ position: 'absolute', left: safe.left, right: safe.right, display: 'flex', flexDirection: 'column', alignItems: 'center', ...place }}>
      <div data-id={id} style={{
        fontFamily: font, fontSize: size, fontWeight: weight, color, textAlign: 'center', lineHeight: 1.15,
        textShadow: '0 4px 18px rgba(0,0,0,0.55)', WebkitTextStroke: `${Math.max(1, size * 0.03)}px rgba(0,0,0,0.6)`,
        paintOrder: 'stroke fill', whiteSpace: 'pre-wrap', ...style,
      }}>
        {page.tokens.map((t, i) => {
          const live = ms >= t.fromMs && ms < t.toMs;
          return (
            <span key={i} style={{ color: live ? highlight : color, display: 'inline-block', transform: live ? 'scale(1.06)' : undefined }}>
              {t.text}
            </span>
          );
        })}
      </div>
    </div>
  );
};
