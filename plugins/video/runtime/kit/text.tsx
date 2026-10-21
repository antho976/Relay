// Text that moves: headlines, words in turn, text fitted to a width, counting numbers and a
// marker sweep. Each takes `id` (its data-id, which video.still's boxes and issues name) and
// `style` for anything else.
import { fitTextOnNLines } from '@remotion/layout-utils';
import React, { useLayoutEffect, useMemo, useState } from 'react';
import { useDelayRender } from 'remotion';
import { mix, rise, stagger, useProgress, useSpring } from './motion';
import { theme } from './theme';

type Common = {
  /** Its data-id. */
  id?: string;
  /** Frames to wait before it enters. */
  delay?: number;
  style?: React.CSSProperties;
};

type Type = {
  /** px; default the theme's title size. */
  size?: number;
  color?: string;
  weight?: number;
  font?: string;
  align?: 'left' | 'center' | 'right';
};

/** A headline that rises in on a spring. */
export const Title: React.FC<Common & Type & { children: React.ReactNode }> = ({
  id, delay = 0, style, size = theme.type.title, color = theme.colors.text, weight = 800, font = theme.fonts.display,
  align = 'center', children,
}) => {
  const p = useSpring(delay);
  return (
    <div data-id={id} style={{
      fontFamily: font, fontSize: size, fontWeight: weight, color, textAlign: align, lineHeight: 1.05,
      letterSpacing: '-0.025em', textWrap: 'balance', ...rise(p, size * 0.4), ...style,
    }}>
      {children}
    </div>
  );
};

/** Words that rise in one after another, `step` frames apart; words in `highlight` take the
 * accent colour. */
export const Words: React.FC<Common & Type & { text: string; step?: number; highlight?: string[]; highlightColor?: string }> = ({
  id, delay = 0, style, size = theme.type.title, color = theme.colors.text, weight = 800, font = theme.fonts.display,
  align = 'center', text, step = 3, highlight = [], highlightColor = theme.colors.accent,
}) => {
  const words = text.split(/\s+/).filter(Boolean);
  const marked = new Set(highlight.map((w) => w.toLowerCase()));
  return (
    <div data-id={id} style={{
      fontFamily: font, fontSize: size, fontWeight: weight, color, textAlign: align, lineHeight: 1.08,
      letterSpacing: '-0.025em', ...style,
    }}>
      {/* The spaces sit between the words, not in them: a line's last space then hangs instead of
          pushing a centred line off centre. */}
      {words.map((word, i) => (
        <React.Fragment key={i}>
          {i > 0 ? ' ' : null}
          <Word delay={stagger(i, step, delay)} distance={size * 0.35}
            color={marked.has(word.toLowerCase().replace(/[^\p{L}\p{N}']/gu, '')) ? highlightColor : undefined}>
            {word}
          </Word>
        </React.Fragment>
      ))}
    </div>
  );
};

const Word: React.FC<{ delay: number; distance: number; color?: string; children: string }> = ({ delay, distance, color, children }) => {
  const p = useSpring(delay);
  return <span style={{ display: 'inline-block', color, ...rise(p, distance) }}>{children}</span>;
};

/** Whether the font `css` names (`800 100px Inter`) is loaded; holds the frame until it is. */
const useFontReady = (css: string): boolean => {
  const { delayRender, continueRender } = useDelayRender();
  const [ready, setReady] = useState(() => typeof document === 'undefined' || document.fonts.check(css));
  useLayoutEffect(() => {
    if (ready) return;
    const handle = delayRender(`Loading the font of ${css}`);
    const done = () => {
      setReady(true);
      continueRender(handle);
    };
    document.fonts.load(css).then(done, done);
  }, [css, ready, delayRender, continueRender]);
  return ready;
};

/** Text sized to fill `width` px on at most `lines` lines (default 1), never above `maxSize`. */
export const FitText: React.FC<Common & Omit<Type, 'size'> & { text: string; width: number; lines?: number; maxSize?: number }> = ({
  id, delay = 0, style, color = theme.colors.text, weight = 800, font = theme.fonts.display, align = 'center',
  text, width, lines = 1, maxSize = 400,
}) => {
  const family = font.split(',')[0].trim();
  const ready = useFontReady(`${weight} 100px ${family}`);
  const size = useMemo(
    () => (ready ? fitTextOnNLines({ text, maxLines: lines, maxBoxWidth: width, fontFamily: font, fontWeight: weight, maxFontSize: maxSize }).fontSize : maxSize),
    [ready, text, lines, width, font, weight, maxSize],
  );
  const p = useSpring(delay);
  return (
    <div data-id={id} style={{
      width, fontFamily: font, fontSize: size, fontWeight: weight, color, textAlign: align, lineHeight: 1.05,
      ...rise(p, size * 0.3), ...style,
    }}>
      {text}
    </div>
  );
};

/** A number that counts from `from` to `to` over `duration` frames: money, users, percentages. */
export const Counter: React.FC<Common & Type & {
  from?: number; to: number; duration?: number; decimals?: number; prefix?: string; suffix?: string;
  /** Default en-US: 1,234.56. */
  locale?: string;
}> = ({
  id, delay = 0, style, size = theme.type.display, color = theme.colors.text, weight = 800, font = theme.fonts.display,
  align = 'center', from = 0, to, duration = 45, decimals = 0, prefix = '', suffix = '', locale = 'en-US',
}) => {
  const p = useProgress({ delay, duration });
  const value = mix(p, from, to);
  const text = new Intl.NumberFormat(locale, { minimumFractionDigits: decimals, maximumFractionDigits: decimals }).format(value);
  return (
    <div data-id={id} style={{
      fontFamily: font, fontSize: size, fontWeight: weight, color, textAlign: align, lineHeight: 1,
      fontVariantNumeric: 'tabular-nums', letterSpacing: '-0.03em', ...style,
    }}>
      {prefix}{text}{suffix}
    </div>
  );
};

/** A marker stroke that sweeps in behind inline words. */
export const Highlight: React.FC<{ children: React.ReactNode; color?: string; delay?: number; duration?: number; style?: React.CSSProperties }> = ({
  children, color = theme.colors.accent, delay = 0, duration = 14, style,
}) => {
  const p = useProgress({ delay, duration });
  return (
    <span style={{
      backgroundImage: `linear-gradient(${color}, ${color})`, backgroundRepeat: 'no-repeat',
      backgroundPosition: '0 88%', backgroundSize: `${p * 100}% 0.36em`, padding: '0 0.08em', ...style,
    }}>
      {children}
    </span>
  );
};
