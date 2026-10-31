// Captions a word at a time, as short-video viewers expect: a few words on screen, the one being
// said lit. From timed words (a transcript with word timings, or an .srt through
// @remotion/captions' parseSrt), or from plain text paced at a speaking rate.
import { createTikTokStyleCaptions, type Caption, type TikTokPage } from '@remotion/captions';
import { fitText } from '@remotion/layout-utils';
import React, { useMemo } from 'react';
import { interpolate, useCurrentFrame, useVideoConfig } from 'remotion';
import { easeOut } from './motion';
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

/** Pages of at most `maxWords` words: a page longer than that is split, each part starting on its
 * first word. */
export const splitPages = (pages: TikTokPage[], maxWords: number): TikTokPage[] => {
  if (maxWords <= 0) return pages;
  const out: TikTokPage[] = [];
  for (const page of pages) {
    const end = page.startMs + page.durationMs;
    for (let i = 0; i < page.tokens.length; i += maxWords) {
      const tokens = page.tokens.slice(i, i + maxWords);
      const next = page.tokens[i + maxWords];
      const startMs = i === 0 ? page.startMs : tokens[0].fromMs;
      const until = next ? next.fromMs : end;
      out.push({ ...page, text: tokens.map((t) => t.text).join(''), tokens: i === 0 ? tokens : [{ ...tokens[0], text: tokens[0].text.trimStart() }, ...tokens.slice(1)], startMs, durationMs: until - startMs });
    }
  }
  return out;
};

/** How the words look:
 * - `lit`: the page in `color`, the word being said in `highlight`.
 * - `box`: the word being said on a `highlight` box, in dark type.
 * - `pop`: words appear as they are said, each popping in.
 * - `plain`: the page in one colour, nothing lit. */
export type CaptionVariant = 'lit' | 'box' | 'pop' | 'plain';

/** Caption pages along the bottom of the safe area (or its middle or top, or centred on `y`, a
 * fraction of the frame's height), the word being said marked by the `variant`. */
export const Captions: React.FC<{
  captions: Caption[];
  /** Its data-id. */
  id?: string;
  /** Words this close together share a page; default 1200 ms. */
  combineWithinMs?: number;
  /** At most this many words on screen at once; default no limit. */
  maxWords?: number;
  variant?: CaptionVariant;
  position?: 'bottom' | 'center' | 'top';
  /** Centre the captions on this fraction of the frame's height (0.36 sits on a split stream's
 * seam); overrides `position`. */
  y?: number;
  uppercase?: boolean;
  size?: number;
  color?: string;
  highlight?: string;
  font?: string;
  weight?: number;
  /** Outline width as a fraction of the size; default 0.03 (0 for none). */
  stroke?: number;
  /** The outline's colour; default a 60% black. */
  outline?: string;
  /** Words always drawn in `highlight` (case and punctuation ignored): the few that carry the line. */
  emphasis?: string[];
  /** Shrink a page whose longest word would run wider than the safe area; default true. Load the
   * font first (loadFonts) so it is measured in its own shapes. */
  fit?: boolean;
  style?: React.CSSProperties;
}> = ({
  captions, id, combineWithinMs = 1200, maxWords = 0, variant = 'lit', position = 'bottom', y, uppercase = false, size = 64, color = '#FFFFFF',
  highlight = theme.colors.accent, font = theme.fonts.display, weight = 800, stroke = 0.03, outline = 'rgba(0,0,0,0.6)', emphasis, fit = true, style,
}) => {
  const frame = useCurrentFrame();
  const { fps, width, height } = useVideoConfig();
  const safe = useSafeArea();
  const pages = useMemo(
    () => splitPages(createTikTokStyleCaptions({ captions, combineTokensWithinMilliseconds: combineWithinMs }).pages, maxWords),
    [captions, combineWithinMs, maxWords],
  );
  const key = (text: string) => text.toLowerCase().replace(/[^\p{L}\p{N}']/gu, '');
  const strong = useMemo(() => new Set((emphasis ?? []).map(key)), [emphasis]);
  const ms = (frame / fps) * 1000;
  const page = pages.find((p) => ms >= p.startMs && ms < p.startMs + p.durationMs);
  if (!page) return null;
  // A word cannot wrap: when the page's longest one is wider than the safe area, the page shrinks.
  const longest = page.tokens.map((t) => t.text.trim()).reduce((a, b) => (b.length > a.length ? b : a), '');
  const room = width - safe.left - safe.right - size * stroke * 2;
  const fitted = fit && longest
    ? Math.min(size, fitText({ text: longest, withinWidth: room, fontFamily: font, fontWeight: weight, textTransform: uppercase ? 'uppercase' : undefined, validateFontIsLoaded: false }).fontSize)
    : size;
  const place: React.CSSProperties = y != null
    ? { top: y * height, transform: 'translateY(-50%)' }
    : position === 'bottom'
      ? { bottom: safe.bottom, justifyContent: 'flex-end' }
      : position === 'top' ? { top: safe.top } : { top: 0, bottom: 0, justifyContent: 'center' };
  const edge = stroke > 0 ? { WebkitTextStroke: `${Math.max(1, fitted * stroke)}px ${outline}`, paintOrder: 'stroke fill' } : {};
  return (
    <div style={{ position: 'absolute', left: safe.left, right: safe.right, display: 'flex', flexDirection: 'column', alignItems: 'center', ...place }}>
      <div data-id={id} style={{
        fontFamily: font, fontSize: fitted, fontWeight: weight, color, textAlign: 'center', lineHeight: 1.15,
        textShadow: '0 4px 18px rgba(0,0,0,0.55)', whiteSpace: 'pre-wrap', textTransform: uppercase ? 'uppercase' : undefined,
        ...edge, ...style,
      }}>
        {page.tokens.map((t, i) => {
          const live = ms >= t.fromMs && ms < t.toMs;
          const base = strong.has(key(t.text)) ? highlight : color;
          if (variant === 'pop') {
            const p = interpolate(ms, [t.fromMs, t.fromMs + 140], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeOut });
            return (
              <span key={i} style={{ display: 'inline-block', whiteSpace: 'pre', opacity: p > 0 ? 1 : 0, transform: `scale(${0.6 + 0.4 * p})`, color: live ? highlight : base }}>
                {t.text}
              </span>
            );
          }
          if (variant === 'box') {
            const lead = t.text.match(/^\s*/)?.[0] ?? '';
            return (
              <span key={i} style={{ whiteSpace: 'pre' }}>
                {lead}
                <span style={{
                  display: 'inline-block', color: live ? '#0B0D12' : base, background: live ? highlight : 'transparent',
                  borderRadius: fitted * 0.18, padding: `0 ${fitted * 0.12}px`, margin: `0 ${-fitted * 0.12}px`,
                  WebkitTextStroke: live ? '0px transparent' : undefined, textShadow: live ? 'none' : undefined,
                }}>{t.text.slice(lead.length)}</span>
              </span>
            );
          }
          const lit = variant === 'lit' && live;
          return (
            <span key={i} style={{ color: lit ? highlight : base, display: 'inline-block', transform: lit ? 'scale(1.06)' : undefined }}>
              {t.text}
            </span>
          );
        })}
      </div>
    </div>
  );
};
