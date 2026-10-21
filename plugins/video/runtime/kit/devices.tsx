// Screens to show an app in, and the hand that uses it: a phone, a desktop window, a tap and a
// pointer that moves and clicks. A screen holds a picture (`src`: staticFile(...) of a screenshot
// or a still from a design) or anything drawn as its children; for a recording, put an
// <OffthreadVideo> in it.
import React from 'react';
import { Img, interpolate, useCurrentFrame } from 'remotion';
import { easeInOut, easeOut } from './motion';

type Screen = {
  /** Its data-id. */
  id?: string;
  /** A picture to fill the screen with. */
  src?: string;
  children?: React.ReactNode;
  style?: React.CSSProperties;
};

const Fill: React.FC<{ src?: string; children?: React.ReactNode }> = ({ src, children }) =>
  src ? <Img src={src} style={{ width: '100%', height: '100%', objectFit: 'cover', display: 'block' }} /> : <>{children}</>;

/** Where a `Phone` `width` px wide draws its screen: the screen's size in px, and the band at its
 * top the camera island covers. A 520 px phone has a 487 × 1093 px screen. */
export const phoneScreen = (width = 520) => {
  const bezel = width * 0.032;
  return { width: width - 2 * bezel, height: Math.round(width * 2.1667) - 2 * bezel, island: width * 0.115 };
};

type Tone = boolean | 'light' | 'dark';
const ink = (tone: Tone) => (tone === 'light' ? '#FFFFFF' : '#0B0D12');

/** A phone's status bar, drawn at iPhone points (390 wide): the time and the signal, Wi-Fi and
 * battery marks, light or dark. */
const StatusBar: React.FC<{ tone: Tone }> = ({ tone }) => {
  const c = ink(tone);
  return (
    <div aria-hidden="true" style={{ position: 'absolute', top: 0, left: 0, right: 0, height: 54, display: 'flex', alignItems: 'center',
      justifyContent: 'space-between', padding: '0 30px 0 44px', color: c, fontFamily: 'Inter, sans-serif', fontWeight: 600, fontSize: 17 }}>
      <span>9:41</span>
      <svg width="78" height="13" viewBox="0 0 78 13" fill={c}>
        <rect x="0" y="8" width="3" height="5" rx="1" /><rect x="5" y="6" width="3" height="7" rx="1" />
        <rect x="10" y="3" width="3" height="10" rx="1" /><rect x="15" y="0" width="3" height="13" rx="1" />
        <path d="M31 4.5a10 10 0 0 1 13 0l-1.6 1.7a7.6 7.6 0 0 0-9.8 0zM34.2 7.9a5.6 5.6 0 0 1 6.6 0L37.5 11.5z" />
        <rect x="51" y="0.5" width="23" height="12" rx="3.5" fill="none" stroke={c} strokeOpacity="0.4" />
        <rect x="53" y="2.5" width="19" height="8" rx="2" />
        <rect x="75.5" y="4.5" width="2" height="4" rx="1" fillOpacity="0.4" />
      </svg>
    </div>
  );
};

/** A modern phone, `width` px wide (default 520) and 19.5:9 tall, its screen showing `src` or the
 * children. With `designWidth` (390 for iPhone points) the children are drawn at that width and
 * scaled to fit the screen, so an app screen is laid out as on a phone: 390 × about 875 points.
 * `statusBar` and `homeIndicator` draw a phone's own marks over it, `'dark'` (for a light app;
 * `true` is dark) or `'light'`. `phoneScreen(width)` gives the screen's size in px. */
export const Phone: React.FC<Screen & { width?: number; designWidth?: number; statusBar?: Tone; homeIndicator?: Tone; frame?: string; screen?: string; shadow?: boolean }> = ({
  id, src, children, style, width = 520, designWidth, statusBar = false, homeIndicator = false, frame = '#111317', screen = '#000', shadow = true,
}) => {
  const height = Math.round(width * 2.1667);
  const bezel = width * 0.032;
  const inner = phoneScreen(width);
  const scale = designWidth ? inner.width / designWidth : 1;
  const points = designWidth ?? inner.width;
  const marks = statusBar || homeIndicator;
  return (
    <div data-id={id} style={{
      position: 'relative', width, height, borderRadius: width * 0.15, background: frame, padding: bezel,
      boxSizing: 'border-box', boxShadow: shadow ? `0 ${width * 0.08}px ${width * 0.2}px rgba(0,0,0,0.45), inset 0 0 0 ${Math.max(1, width * 0.004)}px rgba(255,255,255,0.12)` : undefined,
      ...style,
    }}>
      <div data-relay-screen="" style={{ position: 'relative', width: '100%', height: '100%', borderRadius: width * 0.12, overflow: 'hidden', background: screen }}>
        {designWidth || marks ? (
          <div style={{ position: 'absolute', top: 0, left: 0, width: points, height: inner.height / scale, transform: `scale(${scale})`, transformOrigin: 'top left' }}>
            <Fill src={src}>{children}</Fill>
            {statusBar ? <StatusBar tone={statusBar} /> : null}
            {homeIndicator ? (
              <div style={{ position: 'absolute', bottom: 8, left: '50%', width: 134, height: 5, borderRadius: 3, transform: 'translateX(-50%)', background: ink(homeIndicator) }} />
            ) : null}
          </div>
        ) : (
          <Fill src={src}>{children}</Fill>
        )}
        <div style={{
          position: 'absolute', top: width * 0.03, left: '50%', width: width * 0.3, height: width * 0.085,
          transform: 'translateX(-50%)', borderRadius: width, background: '#000',
        }} />
      </div>
    </div>
  );
};

/** A desktop app or browser window, `width` × `height` px (default 1280 × 800), with a title
 * bar and, given a `url`, an address bar. */
export const Window: React.FC<Screen & { width?: number; height?: number; title?: string; url?: string; dark?: boolean }> = ({
  id, src, children, style, width = 1280, height = 800, title, url, dark = true,
}) => {
  const bar = Math.round(height * 0.055);
  const chrome = dark ? '#1C1F26' : '#ECEEF2';
  const ink = dark ? '#C9CED8' : '#3A3F4A';
  return (
    <div data-id={id} style={{
      width, height, borderRadius: 14, overflow: 'hidden', background: dark ? '#0F1115' : '#FFFFFF',
      boxShadow: '0 40px 100px rgba(0,0,0,0.45), 0 0 0 1px rgba(255,255,255,0.08)', display: 'flex', flexDirection: 'column',
      ...style,
    }}>
      <div style={{ height: bar, flex: 'none', background: chrome, display: 'flex', alignItems: 'center', gap: bar * 0.22, padding: `0 ${bar * 0.45}px` }}>
        {['#FF5F57', '#FEBC2E', '#28C840'].map((c) => (
          <div key={c} style={{ width: bar * 0.26, height: bar * 0.26, borderRadius: bar, background: c }} />
        ))}
        {url ? (
          <div style={{
            marginLeft: bar * 0.4, flex: 1, height: bar * 0.6, borderRadius: bar * 0.3, background: dark ? '#0F1115' : '#FFFFFF',
            color: ink, fontFamily: 'Inter, sans-serif', fontSize: bar * 0.32, display: 'flex', alignItems: 'center', padding: `0 ${bar * 0.35}px`,
          }}>{url}</div>
        ) : title ? (
          <div style={{ flex: 1, textAlign: 'center', color: ink, fontFamily: 'Inter, sans-serif', fontSize: bar * 0.34, fontWeight: 600 }}>{title}</div>
        ) : null}
      </div>
      <div data-relay-screen="" style={{ position: 'relative', flex: 1, overflow: 'hidden' }}>
        <Fill src={src}>{children}</Fill>
      </div>
    </div>
  );
};

/** A tap at (`x`, `y`) — px in whatever it is placed in — at frame `at`: a press and a ripple,
 * `size` px across (default 90). White with a dark edge, seen on light and dark screens alike;
 * `color` changes it. */
export const Tap: React.FC<{ x: number; y: number; at: number; size?: number; color?: string }> = ({
  x, y, at, size = 90, color = 'rgba(255,255,255,0.92)',
}) => {
  const frame = useCurrentFrame() - at;
  if (frame < -4 || frame > 24) return null;
  const press = interpolate(frame, [-4, 0, 6], [0, 1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
  const ring = interpolate(frame, [0, 22], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeOut });
  return (
    <div style={{ position: 'absolute', left: x, top: y, width: 0, height: 0, pointerEvents: 'none' }}>
      <div style={{
        position: 'absolute', left: -size * 0.3, top: -size * 0.3, width: size * 0.6, height: size * 0.6, borderRadius: size,
        background: color, opacity: press * 0.6, transform: `scale(${0.8 + press * 0.2})`,
        boxShadow: '0 0 0 1.5px rgba(0,0,0,0.28), 0 2px 10px rgba(0,0,0,0.25)',
      }} />
      <div style={{
        position: 'absolute', left: -size / 2, top: -size / 2, width: size, height: size, borderRadius: size,
        border: `${size * 0.05}px solid ${color}`, opacity: (1 - ring) * 0.85, transform: `scale(${0.4 + ring * 1.1})`,
        boxShadow: '0 0 0 1px rgba(0,0,0,0.22), inset 0 0 0 1px rgba(0,0,0,0.22)',
      }} />
    </div>
  );
};

export type CursorStop = {
  x: number;
  y: number;
  /** The frame it arrives. */
  at: number;
  /** Click when it arrives. */
  click?: boolean;
};

/** A mouse pointer that glides between `stops` (px in whatever it is placed in) and clicks at
 * those that say so. */
export const Cursor: React.FC<{ stops: CursorStop[]; size?: number; color?: string }> = ({ stops, size = 44, color = '#FFFFFF' }) => {
  const frame = useCurrentFrame();
  if (stops.length === 0) return null;
  const times = stops.map((s) => s.at);
  const opts = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: easeInOut } as const;
  const x = stops.length > 1 ? interpolate(frame, times, stops.map((s) => s.x), opts) : stops[0].x;
  const y = stops.length > 1 ? interpolate(frame, times, stops.map((s) => s.y), opts) : stops[0].y;
  const clicks = stops.filter((s) => s.click).map((s) => frame - s.at);
  const press = Math.max(0, ...clicks.map((d) => interpolate(d, [0, 3, 8], [0, 1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' })));
  return (
    <>
      {stops.filter((s) => s.click).map((s, i) => <Tap key={i} x={s.x} y={s.y} at={s.at} size={size * 1.8} />)}
      <svg width={size} height={size} viewBox="0 0 24 24" style={{
        position: 'absolute', left: x, top: y, transform: `translate(-3px, -2px) scale(${1 - press * 0.15})`, transformOrigin: '3px 2px',
        filter: 'drop-shadow(0 3px 6px rgba(0,0,0,0.45))', pointerEvents: 'none',
      }}>
        <path d="M3 2 L3 19 L7.5 14.8 L10.6 21.5 L13.6 20.2 L10.6 13.6 L16.8 13.6 Z" fill={color} stroke="#111" strokeWidth="1.2" strokeLinejoin="round" />
      </svg>
    </>
  );
};
