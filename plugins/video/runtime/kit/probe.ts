// What video.still sees of a frame (docs/VIDEO.md, "Checks"). Relay's renders set RELAY_PROBE
// for stills; then every frame Remotion seeks to is measured once it has settled, and the
// measurement goes out as one console line the worker reads. Renders of the whole video leave
// it off, so it costs them nothing.
import { continueRender, delayRender } from 'remotion';

declare global {
  interface Window {
    remotion_setFrame?: (frame: number, composition: string, attempt: number) => void;
    remotion_delayRenderTimeouts?: Record<string, unknown>;
    remotion_initialFrame?: number;
  }
}

/** The frame of each composition React last committed, as relayRoot's FrameMark records it. */
let committed: Record<string, number> = {};

/** relayRoot's FrameMark reports the frames its commit drew. */
export const markCommitted = (frames: Record<string, number>) => {
  committed = frames;
};

/** Whether the DOM shows frame `frame` of `composition` yet: Remotion seeks by setting state, and
 * a page that drew an earlier frame (a render's tab goes from frame to frame) keeps showing it
 * until React commits. */
const shows = (composition: string, frame: number) => (committed[composition] ?? window.remotion_initialFrame ?? 0) === frame;

/** The prefix of the console line a measurement goes out on. */
export const PROBE_PREFIX = '__relay_probe__';

const GENERIC = new Set(['serif', 'sans-serif', 'monospace', 'cursive', 'fantasy', 'system-ui', 'ui-serif',
  'ui-sans-serif', 'ui-monospace', 'ui-rounded', 'emoji', 'math', 'fangsong', '-apple-system']);
const MOST_TEXTS = 300;
const MOST_BOXES = 300;

type Rect = [number, number, number, number];

const round = (n: number) => Math.round(n * 10) / 10;
const rect = (r: DOMRect): Rect => [round(r.x), round(r.y), round(r.width), round(r.height)];

const fontChecks = new Map<string, boolean>();
let canvas: CanvasRenderingContext2D | null = null;

/** Whether `family` is a font the browser has, rather than one a fallback stands in for: its
 * words measure the same as two different fallbacks only when it is missing. */
const fontResolved = (family: string, weight: string, style: string): boolean => {
  if (GENERIC.has(family.toLowerCase())) return true;
  const key = `${family}|${weight}|${style}`;
  const known = fontChecks.get(key);
  if (known !== undefined) return known;
  canvas ??= document.createElement('canvas').getContext('2d');
  if (!canvas) return true;
  const sample = 'mmmmmmmmmlli1WQ@#&gyÆ';
  const width = (font: string) => {
    canvas!.font = font;
    return canvas!.measureText(sample).width;
  };
  const named = (fallback: string) => width(`${style} ${weight} 72px "${family}", ${fallback}`);
  const bare = (fallback: string) => width(`${style} ${weight} 72px ${fallback}`);
  const resolved = named('monospace') !== bare('monospace') || named('serif') !== bare('serif');
  fontChecks.set(key, resolved);
  return resolved;
};

/** An element's opacity as drawn: its own and every parent's together. */
const opacityOf = (el: Element): number => {
  let o = 1;
  for (let at: Element | null = el; at && at !== document.body; at = at.parentElement) {
    const s = getComputedStyle(at);
    if (s.display === 'none' || s.visibility === 'hidden') return 0;
    o *= Number.parseFloat(s.opacity || '1');
  }
  return round(o * 100) / 100;
};

/** The nearest box around `el` that hides what overflows it, as the parts that clip meet. */
const clipOf = (el: Element): Rect | null => {
  let clip: [number, number, number, number] | null = null;
  for (let at = el.parentElement; at && at !== document.body; at = at.parentElement) {
    const s = getComputedStyle(at);
    const hides = (v: string) => v !== 'visible';
    if (!hides(s.overflowX) && !hides(s.overflowY) && s.clipPath === 'none') continue;
    const r = at.getBoundingClientRect();
    const [x0, y0, x1, y1] = [r.left, r.top, r.right, r.bottom];
    clip = clip ? [Math.max(clip[0], x0), Math.max(clip[1], y0), Math.min(clip[2], x1), Math.min(clip[3], y1)] : [x0, y0, x1, y1];
  }
  return clip ? [round(clip[0]), round(clip[1]), round(clip[2] - clip[0]), round(clip[3] - clip[1])] : null;
};

const idOf = (el: Element | null): string | null => {
  const tagged = el?.closest('[data-id]') as HTMLElement | null;
  return tagged?.dataset.id ?? null;
};

/** A short, readable way to find an element with no data-id: its tag and classes up two levels. */
const pathOf = (el: Element): string => {
  const parts: string[] = [];
  for (let at: Element | null = el; at && parts.length < 3 && at !== document.body; at = at.parentElement) {
    const cls = typeof at.className === 'string' ? at.className.trim().split(/\s+/).filter(Boolean).slice(0, 2) : [];
    parts.unshift([at.tagName.toLowerCase(), ...cls].join('.'));
  }
  return parts.join(' > ');
};

const measure = (frame: number) => {
  const boxes = Array.from(document.querySelectorAll<HTMLElement>('[data-id]')).slice(0, MOST_BOXES).map((el) => {
    const [x, y, w, h] = rect(el.getBoundingClientRect());
    return { id: el.dataset.id ?? '', x, y, w, h, opacity: opacityOf(el) };
  });

  // Every element that holds words of its own: where they are drawn, in what, and what hides them.
  const owners = new Map<Element, Text[]>();
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node as Text;
    if (!text.data.trim() || !text.parentElement) continue;
    const tag = text.parentElement.tagName;
    if (tag === 'SCRIPT' || tag === 'STYLE' || tag === 'NOSCRIPT') continue;
    const list = owners.get(text.parentElement) ?? [];
    list.push(text);
    owners.set(text.parentElement, list);
  }
  const texts = [];
  const els: Element[] = [];
  for (const [el, nodes] of owners) {
    if (texts.length >= MOST_TEXTS) break;
    const opacity = opacityOf(el);
    if (opacity <= 0.02) continue;
    const range = document.createRange();
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const t of nodes) {
      range.selectNodeContents(t);
      for (const r of Array.from(range.getClientRects())) {
        if (r.width <= 0 || r.height <= 0) continue;
        x0 = Math.min(x0, r.left); y0 = Math.min(y0, r.top);
        x1 = Math.max(x1, r.right); y1 = Math.max(y1, r.bottom);
      }
    }
    if (!Number.isFinite(x0)) continue;
    const s = getComputedStyle(el);
    const family = s.fontFamily.split(',')[0].trim().replace(/^["']|["']$/g, '');
    // Its size as drawn: a phone's screen scaled to fit, a pop's scale, all count.
    const html = el as HTMLElement;
    const drawnScale = html.offsetHeight > 0 ? el.getBoundingClientRect().height / html.offsetHeight : 1;
    texts.push({
      id: idOf(el),
      path: pathOf(el),
      text: nodes.map((t) => t.data).join('').replace(/\s+/g, ' ').trim().slice(0, 60),
      x: round(x0), y: round(y0), w: round(x1 - x0), h: round(y1 - y0),
      size: round(Number.parseFloat(s.fontSize) * (Number.isFinite(drawnScale) && drawnScale > 0 ? drawnScale : 1)),
      weight: s.fontWeight,
      family,
      resolved: fontResolved(family, s.fontWeight, s.fontStyle),
      color: s.webkitTextFillColor && s.webkitTextFillColor !== s.color ? s.webkitTextFillColor : s.color,
      opacity,
      clip: clipOf(el),
      within: [] as number[],
      // Not meant to be read (aria-hidden: a status bar's clock), or part of an app's screen in a
      // device frame (data-relay-screen): context the video's own words carry.
      decorative: !!el.closest('[aria-hidden="true"]'),
      screen: !!el.closest('[data-relay-screen]'),
    });
    els.push(el);
  }
  // Which other texts each sits inside (a <b> in a sentence): those overlap it by design.
  for (let i = 0; i < els.length; i++) {
    for (let j = 0; j < els.length; j++) {
      if (i !== j && els[j].contains(els[i])) texts[i].within.push(j);
    }
  }

  const images = Array.from(document.images)
    .filter((img) => img.complete && img.naturalWidth === 0 && opacityOf(img) > 0.02)
    .map((img) => ({ id: idOf(img), src: img.currentSrc || img.src }));

  // A render's page is exactly the composition's size: its viewport is the frame.
  console.log(PROBE_PREFIX + JSON.stringify({
    frame,
    width: window.innerWidth,
    height: window.innerHeight,
    boxes,
    texts,
    images,
  }));
};

let installed = false;

/** Measure every frame Remotion seeks to while RELAY_PROBE is set. Remotion's own seek holds a
 * delayRender until React has drawn the frame; this one holds another until every other hold has
 * gone (pictures and fonts loaded), so the measurement is of the frame that is captured. */
export const installProbe = () => {
  if (installed || typeof window === 'undefined' || process.env.RELAY_PROBE !== '1') return;
  installed = true;
  let seek: Window['remotion_setFrame'];
  Object.defineProperty(window, 'remotion_setFrame', {
    configurable: true,
    get: () => (frame: number, composition: string, attempt: number) => {
      seek?.(frame, composition, attempt);
      const handle = delayRender('Relay measuring the frame', { timeoutInMilliseconds: 25000 });
      const started = performance.now();
      const settle = () => {
        const holds = Object.keys(window.remotion_delayRenderTimeouts ?? {}).length;
        if ((holds > 1 || !shows(composition, frame)) && performance.now() - started < 20000) {
          requestAnimationFrame(settle);
          return;
        }
        try {
          measure(frame);
        } finally {
          continueRender(handle);
        }
      };
      requestAnimationFrame(settle);
    },
    set: (fn: Window['remotion_setFrame']) => {
      seek = fn;
    },
  });
};
