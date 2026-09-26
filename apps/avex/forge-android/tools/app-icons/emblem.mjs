// The Avex "A" emblem, drawn once in the 108-unit adaptive-icon grid.
//
// The mark is one open pentagram: a caret (the A's legs) plus two long diagonals that start as spurs
// outside the legs and run down to the opposite foot. Walked as a single polyline it is
//   right spur -> left foot -> apex -> right foot -> left spur
// and the two diagonals cross under the apex, where the second one breaks around the first so the
// crossing reads as a woven knot rather than a blot.
//
// Everything that draws the mark (the launcher foregrounds, the monochrome layer, the notification
// glyph, the picker previews) takes its geometry from here, so the variants cannot drift apart.

export const GRID = 108;

// Centre-line points. The apex and feet sit inside the 66-unit safe circle once the stroke is added.
const APEX = [54, 36.5];
const FOOT_L = [35.6, 70.5];
const FOOT_R = [72.4, 70.5];
const SPUR_L = [38.4, 52.2];
const SPUR_R = [69.6, 52.2];

export const STROKE = 4.6;
// The gap either side of the over-strand, along the under-strand, beyond the over-strand's own edge.
const WEAVE_GAP = 1.5;

const sub = (a, b) => [a[0] - b[0], a[1] - b[1]];
const add = (a, b) => [a[0] + b[0], a[1] + b[1]];
const mul = (a, k) => [a[0] * k, a[1] * k];
const len = (a) => Math.hypot(a[0], a[1]);
const unit = (a) => mul(a, 1 / len(a));
const fmt = (p) => `${+p[0].toFixed(3)} ${+p[1].toFixed(3)}`;

function intersect(p1, p2, p3, p4) {
  const d = (p1[0] - p2[0]) * (p3[1] - p4[1]) - (p1[1] - p2[1]) * (p3[0] - p4[0]);
  const t = ((p1[0] - p3[0]) * (p3[1] - p4[1]) - (p1[1] - p3[1]) * (p3[0] - p4[0])) / d;
  return add(p1, mul(sub(p2, p1), t));
}

/** Transform a point about the grid centre: scale then offset (used for the notification glyph). */
function place(p, scale, dx, dy) {
  return [(p[0] - 54) * scale + 54 + dx, (p[1] - 54) * scale + 54 + dy];
}

/**
 * The mark as stroke paths, stroked with [STROKE], miter joins, butt caps. `main` is the whole
 * polyline; with `weave` it stops at the crossing and `tail` is the rest of the under strand.
 */
export function emblemPaths({ scale = 1, dx = 0, dy = 0, weave = false } = {}) {
  const P = (p) => place(p, scale, dx, dy);
  const cross = intersect(SPUR_R, FOOT_L, FOOT_R, SPUR_L);
  const underDir = unit(sub(SPUR_L, FOOT_R));
  const overDir = unit(sub(FOOT_L, SPUR_R));
  const sin = Math.abs(underDir[0] * overDir[1] - underDir[1] * overDir[0]);
  const half = STROKE / 2 / sin + WEAVE_GAP;
  const gapStart = sub(cross, mul(underDir, half));
  const gapEnd = add(cross, mul(underDir, half));
  if (!weave) {
    return { main: `M${fmt(P(SPUR_R))} L${fmt(P(FOOT_L))} L${fmt(P(APEX))} L${fmt(P(FOOT_R))} L${fmt(P(SPUR_L))}`, tail: '' };
  }
  return {
    main: `M${fmt(P(SPUR_R))} L${fmt(P(FOOT_L))} L${fmt(P(APEX))} L${fmt(P(FOOT_R))} L${fmt(P(gapStart))}`,
    tail: `M${fmt(P(gapEnd))} L${fmt(P(SPUR_L))}`,
  };
}

export const MITER = 4;

// The spur tips are chisel points, as the original mark had: the spur's top edge runs straight out to
// the point and its underside cuts back at an angle. Length of the point, in stroke widths.
const TIP = 1.5;

/** The two chisel tips as closed polygons, for a stroke of width `w`. */
export function tipPaths({ scale = 1, dx = 0, dy = 0, width = STROKE } = {}) {
  const P = (p) => place(p, scale, dx, dy);
  const h = width / 2;
  return [[SPUR_R, FOOT_L], [SPUR_L, FOOT_R]].map(([end, toward]) => {
    const inward = unit(sub(toward, end));
    let n = [inward[1], -inward[0]];
    if (n[1] > 0) n = mul(n, -1); // the upper side
    const top = add(end, mul(n, h)), bottom = sub(end, mul(n, h));
    const tip = add(top, mul(inward, -TIP * width));
    // Overlap the stroke by a hair so anti-aliasing leaves no seam where the two meet.
    const lap = mul(inward, 0.4);
    return `M${fmt(P(add(top, lap)))} L${fmt(P(top))} L${fmt(P(tip))} L${fmt(P(bottom))} L${fmt(P(add(bottom, lap)))} Z`;
  });
}

/** SVG stroke attributes for the mark at a given scale. */
export function strokeAttrs(scale = 1, width = STROKE) {
  return `fill="none" stroke-width="${+(width * scale).toFixed(3)}" stroke-linejoin="miter" stroke-miterlimit="${MITER}" stroke-linecap="butt"`;
}

/**
 * The mark as SVG carrying `extra` attributes (stroke colour, filter, …). The body is stroked; the
 * tips are filled with the same paint, so `stroke*` attributes are mirrored onto `fill*` for them.
 * Grouped so a filter or opacity applies to the whole mark at once.
 */
export function emblemSvg(extra = '', opts = {}) {
  const { main, tail } = emblemPaths(opts);
  const width = opts.strokeOverride ?? STROKE;
  const s = strokeAttrs(opts.scale ?? 1, width);
  const paint = extra.replace(/filter="[^"]*"/, '');
  const fillPaint = paint.replace(/stroke-opacity=/g, 'fill-opacity=').replace(/stroke=/g, 'fill=');
  const filter = (extra.match(/filter="[^"]*"/) || [''])[0];
  const tips = tipPaths({ scale: opts.scale ?? 1, dx: opts.dx ?? 0, dy: opts.dy ?? 0, width })
    .map((d) => `<path d="${d}" ${fillPaint}/>`).join('');
  return `<g ${filter}><path d="${main}" ${s} ${paint}/>` + (tail ? `<path d="${tail}" ${s} ${paint}/>` : '') + tips + '</g>';
}
