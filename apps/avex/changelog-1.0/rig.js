const CH = {
  nori: {
    ink: '#2B2320', cheek: '#F4A3A8', heart: '#E23D3D', tongue: '#F27C88',
    top: 64, eyeY: 118, eyeDX: 16, mouthY: 132, cheekY: 128, cheekDX: 29,
    shoulder: [40, 144], arm: { len: 20, width: 12, color: '#E6DFD1' },
    hip: [18, 164], leg: { len: 12, width: 12, color: '#D4CCBB' },
    back: () => '',
    body: () => `<path d="M100 64 C 114 64, 156 130, 154 150 C 152 168, 136 170, 100 170 C 64 170, 48 168, 46 150 C 44 130, 86 64, 100 64 Z" fill="#F6F3EC" stroke="#F6F3EC" stroke-width="4" stroke-linejoin="round"/>
      <g fill="#E4DDD0"><ellipse cx="84" cy="96" rx="2.5" ry="1.5" transform="rotate(-30 84 96)"/><ellipse cx="118" cy="104" rx="2.5" ry="1.5" transform="rotate(25 118 104)"/><ellipse cx="66" cy="140" rx="2.5" ry="1.5"/><ellipse cx="136" cy="136" rx="2.5" ry="1.5" transform="rotate(-20 136 136)"/></g>
      <path d="M80 148 L120 148 L120 171 L80 171 Z" fill="#26332C" stroke="#26332C" stroke-width="4" stroke-linejoin="round"/>
      <ellipse cx="86" cy="86" rx="6" ry="10" transform="rotate(35 86 86)" fill="#FFFFFF" opacity=".7"/>`,
  },
  mochi: {
    ink: '#2B2320', cheek: '#F4A3A8', heart: '#E23D3D', tongue: '#F27C88',
    top: 48, eyeY: 120, eyeDX: 18, mouthY: 136, cheekY: 130, cheekDX: 32,
    shoulder: [44, 132], arm: { len: 20, width: 13, color: '#E3D5BF' },
    hip: [20, 162], leg: { len: 13, width: 13, color: '#D6C6AE' },
    back: (w, s = {}) => `<g transform="rotate(${w} 100 72) translate(100 74) scale(${s.grow || 1}) translate(-100 -74)">
        <path d="M100 74 C 100 64, 100 60, 102 54" fill="none" stroke="#7DB074" stroke-width="3.5" stroke-linecap="round"/>
        <ellipse cx="92" cy="54" rx="9" ry="5" transform="rotate(-25 92 54)" fill="#8CC084"/>
        <ellipse cx="111" cy="50" rx="10" ry="5.5" transform="rotate(20 111 50)" fill="#8CC084"/></g>`,
    body: () => `<path d="M50 162 C 44 108, 64 70, 100 70 C 136 70, 156 108, 150 162 C 130 172, 70 172, 50 162 Z" fill="#F4ECDF"/>
      <ellipse cx="76" cy="92" rx="8" ry="12" transform="rotate(35 76 92)" fill="#FFFFFF" opacity=".6"/>`,
  },
  kettle: {
    ink: '#2A0E0E', cheek: '#FFB3B3', heart: '#FFE3E3', tongue: '#FF8F9A',
    top: 50, eyeY: 116, eyeDX: 19, mouthY: 134, cheekY: 128, cheekDX: 34,
    shoulder: [46, 126], arm: { len: 24, width: 12, color: '#C22F2F' },
    hip: [18, 160], leg: { len: 14, width: 11, color: '#A62B2B' },
    back: () => `<path d="M74 92 C 68 38, 132 38, 126 92" fill="none" stroke="#E23D3D" stroke-width="14" stroke-linecap="round"/>`,
    body: () => `<rect x="70" y="156" width="60" height="12" rx="6" fill="#A62B2B"/>
      <ellipse cx="100" cy="120" rx="52" ry="46" fill="#E23D3D"/>
      <ellipse cx="74" cy="100" rx="7" ry="12" transform="rotate(35 74 100)" fill="#FFFFFF" opacity=".35"/>`,
  },
  momo: {
    ink: '#3A1A14', cheek: '#EE6F62', heart: '#E23D3D', tongue: '#E8616A',
    top: 56, eyeY: 122, eyeDX: 18, mouthY: 138, cheekY: 132, cheekDX: 32,
    shoulder: [44, 134], arm: { len: 20, width: 12, color: '#EE937D' },
    hip: [18, 162], leg: { len: 13, width: 12, color: '#DE806B' },
    back: w => `<g transform="rotate(${w} 100 82)">
        <path d="M100 82 C 100 74, 101 68, 104 62" fill="none" stroke="#6E8F4E" stroke-width="4" stroke-linecap="round"/>
        <ellipse cx="116" cy="62" rx="13" ry="6" transform="rotate(-18 116 62)" fill="#86B05E"/></g>`,
    body: () => `<path d="M100 82 C 86 66, 50 72, 50 120 C 50 152, 72 170, 100 170 C 128 170, 150 152, 150 120 C 150 72, 114 66, 100 82 Z" fill="#F7A58F"/>
      <path d="M100 82 C 97 88, 97 94, 99 100" fill="none" stroke="#E4846E" stroke-width="3" stroke-linecap="round"/>
      <ellipse cx="74" cy="98" rx="8" ry="13" transform="rotate(35 74 98)" fill="#FFFFFF" opacity=".35"/>`,
  },
};

const MOOD = {
  happy:  { eyes: 'open',   mouth: 'smile',  blush: .45 },
  cheer:  { eyes: 'happy',  mouth: 'grin',   blush: .8 },
  love:   { eyes: 'heart',  mouth: 'smile',  blush: .9 },
  sleepy: { eyes: 'closed', mouth: 'osmall', blush: .35 },
  sad:    { eyes: 'sad',    mouth: 'frown',  blush: .2 },
  shock:  { eyes: 'wide',   mouth: 'o',      blush: .2 },
  calm:   { eyes: 'open',   mouth: 'osmall', blush: .35 },
  dizzy:  { eyes: 'dizzy',  mouth: 'o',      blush: .25 },
  strain: { eyes: 'squeeze', mouth: 'grit',  blush: .95 },
};

const sk = (c, w) => `fill="none" stroke="${c}" stroke-width="${w}" stroke-linecap="round" stroke-linejoin="round"`;
const HEART = 'M0 7 C -12 -1, -9 -13, 0 -6 C 9 -13, 12 -1, 0 7 Z';
const STAR = 'M0 -7 L1.8 -1.8 L7 0 L1.8 1.8 L0 7 L-1.8 1.8 L-7 0 L-1.8 -1.8 Z';

function eyesSVG(c, type, s) {
  const [lx, ly] = s.look || [0, 0], bl = s.blink || 0;
  return [-1, 1].map(side => {
    let g = '';
    switch (type) {
      case 'open':   g = `<ellipse rx="6.5" ry="8.5" fill="${c.ink}"/><circle cx="2" cy="-3.2" r="2.3" fill="#fff"/>`; break;
      case 'happy':  g = `<path d="M-7 3 Q0 -7 7 3" ${sk(c.ink, 3.8)}/>`; break;
      case 'closed': g = `<path d="M-7 -1 Q0 5 7 -1" ${sk(c.ink, 3.4)}/>`; break;
      case 'sad':    g = `<ellipse cy="2" rx="5.5" ry="7" fill="${c.ink}"/><circle cx="1.8" cy="-1" r="2" fill="#fff"/><path d="${side < 0 ? 'M-8 -9 L7 -14' : 'M8 -9 L-7 -14'}" ${sk(c.ink, 3)}/>`; break;
      case 'wide':   g = `<circle r="9" fill="${c.ink}"/><circle cx="3" cy="-3" r="3.4" fill="#fff"/><circle cx="-3" cy="3.5" r="1.4" fill="#fff"/><path d="M-7 -16 Q0 -20 7 -16" ${sk(c.ink, 3)}/>`; break;
      case 'dizzy':  g = `<g transform="rotate(${((s.time || 0) * 540 * side).toFixed(1)})"><path d="M0 0 m-1 0 a1 1 0 1 1 2 0 a2.5 2.5 0 1 1 -5 0 a4 4 0 1 1 8 0 a5.5 5.5 0 1 1 -11 0" ${sk(c.ink, 2.2)}/></g>`; break;
      case 'squeeze': g = `<path d="${side < 0 ? 'M-6 -5 L5 0 L-6 5' : 'M6 -5 L-5 0 L6 5'}" ${sk(c.ink, 3.4)}/>`; break;
      case 'heart':  g = `<g transform="scale(${1.1 + .12 * Math.abs(Math.sin((s.time || 0) * 5))})"><path d="${HEART}" fill="${c.heart}"/></g>`; break;
    }
    const blinkable = type === 'open' || type === 'wide' || type === 'sad';
    const ky = blinkable ? Math.max(.08, 1 - bl) : 1;
    const moveEyes = blinkable ? `${lx} ${ly}` : '0 0';
    return `<g transform="translate(${100 + side * c.eyeDX} ${c.eyeY}) translate(${moveEyes}) scale(1 ${ky})">${g}</g>`;
  }).join('');
}
function mouthSVG(c, type) {
  const m = {
    smile:  `<path d="M-7 -2 Q0 6 7 -2" ${sk(c.ink, 3.5)}/>`,
    grin:   `<path d="M-10 -3 L10 -3 Q10 10 0 10 Q-10 10 -10 -3 Z" fill="${c.ink}" stroke="${c.ink}" stroke-width="1.5" stroke-linejoin="round"/><ellipse cy="6" rx="5" ry="3" fill="${c.tongue}"/>`,
    o:      `<ellipse rx="5" ry="6.5" fill="${c.ink}"/>`,
    osmall: `<ellipse cy="1" rx="3" ry="3.5" fill="${c.ink}"/>`,
    frown:  `<path d="M-7 3 Q0 -4 7 3" ${sk(c.ink, 3.5)}/>`,
    grit:   `<rect x="-9" y="-5" width="18" height="10" rx="3" fill="#fff" stroke="${c.ink}" stroke-width="2.5"/><path d="M-3 -5 V5 M3 -5 V5" stroke="${c.ink}" stroke-width="1.8"/>`,
  }[type];
  return `<g transform="translate(100 ${c.mouthY})">${m}</g>`;
}
function fxSVG(c, kind, t) {
  switch (kind) {
    case 'sparkles': return [[34, 72, 0], [166, 58, .3], [156, 112, .6], [46, 122, .15]].map(([x, y, d], i) => {
      const k = .55 + .55 * Math.abs(Math.sin(2 * Math.PI * (t * 1.4 + d)));
      return `<g transform="translate(${x} ${y}) rotate(${t * 90 + i * 30}) scale(${k})"><path d="${STAR}" fill="#FFE08A"/></g>`;
    }).join('');
    case 'hearts': return [[150, 70], [50, 84], [138, 44]].map(([x, y], i) => {
      const p = (t * .55 + i / 3) % 1;
      return `<g transform="translate(${x} ${y - p * 34}) scale(.8)" opacity="${Math.sin(Math.PI * p).toFixed(3)}"><path d="${HEART}" fill="#E23D3D"/></g>`;
    }).join('');
    case 'zzz': return [[136, 72, 13], [150, 58, 16], [165, 42, 19]].map(([x, y, s], i) => {
      const p = (t * .5 + i / 3) % 1;
      return `<text x="${x + p * 10}" y="${y - p * 24}" opacity="${Math.sin(Math.PI * p).toFixed(3)}" font-family="IBM Plex Mono, monospace" font-weight="500" font-size="${s}" fill="#BFB6AA">z</text>`;
    }).join('');
    case 'shock': return `<g transform="translate(${100 + c.eyeDX + 30} ${c.eyeY - 30})"><path d="M0 0 C -5 7, -5 12, 0 12 C 5 12, 5 7, 0 0 Z" fill="#8FC7F2"/></g>
      <g ${sk('#F2EFEA', 3)} opacity=".85"><path d="M100 ${c.top - 10} L100 ${c.top - 22} M84 ${c.top - 6} L77 ${c.top - 16} M116 ${c.top - 6} L123 ${c.top - 16}"/></g>`;
    case 'sweat': return [[-1, 0], [1, .5]].map(([side, off]) => {
      const p = (t * 1.6 + off) % 1;
      return `<g transform="translate(${100 + side * (52 + p * 26)} ${c.eyeY - 30 + p * p * 40})" opacity="${(1 - p).toFixed(3)}"><path d="M0 0 C -4 6, -4 10, 0 10 C 4 10, 4 6, 0 0 Z" fill="#8FC7F2"/></g>`;
    }).join('');
    case 'stars': return [0, 1, 2].map(i => {
      const a = (t * 4 + i * 2.09), x = 100 + Math.cos(a) * 34, y = c.top - 6 + Math.sin(a) * 9;
      return `<g transform="translate(${x.toFixed(1)} ${y.toFixed(1)}) scale(.9)"><path d="${STAR}" fill="#FFE08A"/></g>`;
    }).join('');
    default: return '';
  }
}
function drawChar(c, s) {
  const m = MOOD[s.mood || 'happy'];
  const sx = s.sx ?? 1, sy = s.sy ?? 1, dy = s.dy ?? 0, rot = s.rot ?? 0;
  const lift = clamp(1 + dy / 90, .45, 1);
  const rig = `translate(0 ${dy.toFixed(2)}) translate(100 182) rotate(${rot.toFixed(2)}) scale(${sx.toFixed(4)} ${sy.toFixed(4)}) translate(-100 -182)`;
  if (s.eyesOnly) return `<svg viewBox="0 0 200 200"><g transform="${rig}">${eyesSVG(c, 'open', s)}</g></svg>`;
  const arm = c.arm, leg = c.leg;
  // Arms can be lengthened and thickened for a prop, and end in fists that grip it.
  const aL = arm.len * (s.armK || 1), aW = arm.width * (s.armW || 1);
  const fist = s.fists ? `<circle cx="0" cy="${aL}" r="${(aW * .74).toFixed(2)}" fill="${arm.color}" stroke="${c.ink}" stroke-opacity=".28" stroke-width="1.6"/>` : '';
  const arms = [[1, s.armR ?? 18], [-1, s.armL ?? 18]].map(([side, a]) =>
    `<g transform="translate(${100 + side * c.shoulder[0]} ${c.shoulder[1]}) scale(${side} 1) rotate(${(-a).toFixed(2)})"><line x1="0" y1="0" x2="0" y2="${aL}" stroke="${arm.color}" stroke-width="${aW}" stroke-linecap="round"/>${fist}</g>`).join('');
  const legs = [[1, s.legR || 0], [-1, s.legL || 0]].map(([side, a]) =>
    `<g transform="translate(${100 + side * c.hip[0]} ${c.hip[1]}) rotate(${a.toFixed(2)})"><line x1="0" y1="0" x2="0" y2="${leg.len}" stroke="${leg.color}" stroke-width="${leg.width}" stroke-linecap="round"/></g>`).join('');
  const cheeks = [-1, 1].map(d => `<ellipse cx="${100 + d * c.cheekDX}" cy="${c.cheekY}" rx="7" ry="4" fill="${c.cheek}" opacity="${m.blush}"/>`).join('');
  const w = s.wig || 0;
  const glasses = s.glasses ? `<g stroke="${c.ink}" stroke-width="2.4" fill="rgba(255,255,255,.2)"><circle cx="${100 - c.eyeDX}" cy="${c.eyeY}" r="11"/><circle cx="${100 + c.eyeDX}" cy="${c.eyeY}" r="11"/><path d="M${100 - c.eyeDX + 11} ${c.eyeY - 1} Q 100 ${c.eyeY - 5} ${100 + c.eyeDX - 11} ${c.eyeY - 1}" fill="none"/></g>` : '';
  return `<svg viewBox="0 0 200 200">
    <ellipse cx="100" cy="184" rx="${(42 * lift).toFixed(2)}" ry="${(5 * lift).toFixed(2)}" fill="#000" opacity="${(.45 * lift * (s.shadow ?? 1)).toFixed(3)}"/>
    <g transform="${rig}">${c.back(w, s)}${legs}${c.body(w, s)}${cheeks}${eyesSVG(c, m.eyes, s)}${glasses}${mouthSVG(c, m.mouth)}${s.prop || ''}${arms}${s.front || ''}</g>
    ${s.fx ? `<g transform="translate(0 ${dy.toFixed(2)})">${fxSVG(c, s.fx, s.time || 0)}</g>` : ''}</svg>`;
}

/* Motion vocabulary. Each returns part of a character state; scenes merge them. */
const Act = {
  blink: (t, seed = 0) => { const p = (t + seed) % 3.7; return p < .16 ? Math.sin(Math.PI * p / .16) : 0; },
  breathe: (t, amp = .018) => { const k = Math.sin(2 * Math.PI * t / BAR); return { sx: 1 - amp * .6 * k, sy: 1 + amp * k }; },
  bounce: (t, amp = 5) => {
    const p = beatPhase(t), land = p < .16 ? 1 - p / .16 : 0;
    return { dy: -Math.sin(Math.PI * p) * amp, sx: 1 + .05 * land, sy: 1 - .06 * land };
  },
  // Jump once every two beats: squash, fly, land, settle.
  cheer: (t, h = 30, off = 0) => {
    const p = ((((t + off) / (2 * BEAT)) % 1) + 1) % 1;
    let dy = 0, sx = 1, sy = 1;
    if (p < .1) { const k = p / .1; sx = 1 + .1 * k; sy = 1 - .12 * k; }
    else if (p < .7) { const k = (p - .1) / .6; dy = -4 * h * k * (1 - k); sx = 1 - .06 * Math.sin(Math.PI * k); sy = 1 + .1 * Math.sin(Math.PI * k); }
    else if (p < .85) { const k = (p - .7) / .15; sx = 1 + .1 * Math.sin(Math.PI * k); sy = 1 - .12 * Math.sin(Math.PI * k); }
    const pump = 145 + 14 * Math.sin(2 * Math.PI * (t + off) * 2);
    return { dy, sx, sy, armR: pump, armL: pump };
  },
  wave: t => ({ armR: 140 + 24 * Math.sin(2 * Math.PI * t * 2.2) }),
  walk: (t, rate = 2) => {
    const ph = 2 * Math.PI * t * rate / 2;
    return { legR: 24 * Math.sin(ph), legL: -24 * Math.sin(ph), dy: -Math.abs(Math.sin(ph)) * 5, armR: 20 - 18 * Math.sin(ph), armL: 20 + 18 * Math.sin(ph), rot: 2.5 * Math.sin(ph) };
  },
  // One arc from the ground, for arrivals and hops.
  hop: (s, dur = .55, h = 30) => {
    if (s < 0 || s > dur + .2) return {};
    if (s > dur) { const k = (s - dur) / .2; return { sx: 1 + .08 * Math.sin(Math.PI * k), sy: 1 - .1 * Math.sin(Math.PI * k) }; }
    const k = s / dur; return { dy: -4 * h * k * (1 - k), sx: 1 - .05 * Math.sin(Math.PI * k), sy: 1 + .08 * Math.sin(Math.PI * k) };
  },
};
const wig = (t, speed = 1.6, amp = 7) => amp * Math.sin(2 * Math.PI * t * speed / 2);

function mkChar(parent, def, size) {
  const d = document.createElement('div');
  d.className = 'char';
  d.style.width = d.style.height = size + 'px';
  parent.appendChild(d);
  return {
    d,
    set(x, y, s = {}, o = 1, scale = 1) {
      if (o <= 0) { d.style.display = 'none'; return; }
      d.style.display = 'block';
      d.style.opacity = o;
      d.style.transform = `translate(${x - size / 2}px, ${y - size * .91}px) scale(${scale})`;
      d.style.transformOrigin = `50% 91%`;
      d.innerHTML = drawChar(def, s);
    },
  };
}

/* ═══ The mark ═════════════════════════════════════════════════════════════════════════════════
 * Traced from play-store-icon-512.png: the A and the two bands that cross inside it and break out
 * past its legs. Coordinates are in that icon's 512 space.
 */
const MARK = [
  '255.5,164 371,349 342,333 255.5,194.4 169,333 140,349',
  '176,246 205,251 360.2,331.7 371,349 349,344.4 195,264.3',
  '335,246 306,251 150.8,331.7 140,349 162,344.4 316,264.3',
];
function markSVG(fill = 'var(--ink)') {
  return `<svg viewBox="130 150 252 212" style="overflow:visible;width:100%;height:100%;display:block">
    ${MARK.map(p => `<polygon class="mk-fill" points="${p}" fill="${fill}"/>`).join('')}
    ${MARK.map(p => `<polygon class="mk-line" points="${p}" fill="none" stroke="${fill}" stroke-width="2.4" stroke-linejoin="round" pathLength="1" stroke-dasharray="1 1"/>`).join('')}
  </svg>`;
}
function drawMark(root, draw, fill) {
  root.querySelectorAll('.mk-line').forEach((l, i) => {
    const k = E.inOut(clamp(draw * 1.35 - i * .18));
    l.style.strokeDashoffset = 1 - k;
    l.style.opacity = 1 - fill;
  });
  root.querySelectorAll('.mk-fill').forEach(f => { f.style.opacity = fill; });
}
