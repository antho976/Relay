// Every selectable launcher icon, as SVG layers on the 108-unit adaptive grid.
//
// Each icon is the same Avex mark (emblem.mjs) with its own colour, material and backdrop. An icon
// supplies two layers, the way Android composes an adaptive icon:
//   bg: full-bleed, 108×108, opaque. The launcher masks it and shifts it for parallax.
//   fg: the mark alone on transparency, so it floats over the backdrop.
// Keys match the `AppIcon` enum names in app/src/main/java/com/forge/app/appicon/AppIcon.kt.

import { emblemSvg } from './emblem.mjs';

// ── helpers ────────────────────────────────────────────────────────────────────────────────────

/** Deterministic PRNG (mulberry32) so a re-run reproduces the same stars and facets. */
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const FULL = 'filterUnits="userSpaceOnUse" x="-20" y="-20" width="148" height="148"';

/** A mask whose white is the mark; paint anything inside `<g mask>` to fill only the mark. */
const markMask = (id, opts = {}) =>
  `<mask id="${id}" maskUnits="userSpaceOnUse" x="0" y="0" width="108" height="108">${emblemSvg('stroke="#fff"', opts)}</mask>`;

/** Film grain across the whole layer. `k` is its strength. */
const grain = (id, k = 0.06, freq = 0.9, seed = 3) => ({
  defs: `<filter id="${id}" ${FULL}><feTurbulence type="fractalNoise" baseFrequency="${freq}" numOctaves="2" seed="${seed}"/>
         <feColorMatrix values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 ${k * 2} -${k * 0.9}"/></filter>`,
  body: `<rect width="108" height="108" filter="url(#${id})"/>`,
});

/** Scattered stars; bright ones get a soft halo. */
function stars(seed, n, color = '#fff', maxR = 0.55) {
  const r = rng(seed);
  let out = '';
  for (let i = 0; i < n; i++) {
    const x = r() * 108, y = r() * 108, s = 0.12 + r() ** 3 * maxR, o = 0.35 + r() * 0.65;
    out += `<circle cx="${x.toFixed(2)}" cy="${y.toFixed(2)}" r="${s.toFixed(2)}" fill="${color}" opacity="${o.toFixed(2)}"/>`;
    if (s > maxR * 0.7) out += `<circle cx="${x.toFixed(2)}" cy="${y.toFixed(2)}" r="${(s * 3).toFixed(2)}" fill="${color}" opacity=".12"/>`;
  }
  return out;
}

/** A soft drop shadow under the mark so it lifts off busy backdrops. */
const liftShadow = (id, alpha = 0.45, blur = 1.6, dy = 1.2) => ({
  defs: `<filter id="${id}" ${FULL}><feGaussianBlur stdDeviation="${blur}"/><feOffset dy="${dy}"/>
         <feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 ${alpha} 0"/></filter>`,
  body: emblemSvg(`stroke="#000" filter="url(#${id})"`),
});

/** The rim of the mark facing (dx, dy): the mark minus itself shifted away from that side. */
const bevelMask = (id, dx, dy) =>
  `<mask id="${id}" maskUnits="userSpaceOnUse" x="0" y="0" width="108" height="108">${emblemSvg('stroke="#fff"')}
     <g transform="translate(${-dx} ${-dy})">${emblemSvg('stroke="#000"')}</g></mask>`;

// ── families ───────────────────────────────────────────────────────────────────────────────────

/** Avex: the house icons. Warm Pearl plate, a faint overhead glow, the mark in bone or signal red. */
function avex(key, label, mark) {
  const g = grain(`${key}-g`, 0.035);
  const sh = liftShadow(`${key}-s`, 0.5);
  return {
    key, family: 'Avex', label,
    bg: `<defs><linearGradient id="${key}-b" x1="0" y1="0" x2="0" y2="1">
           <stop offset="0" stop-color="#1D1813"/><stop offset="1" stop-color="#0A0806"/></linearGradient>
           <radialGradient id="${key}-h" cx="54" cy="30" r="60" gradientUnits="userSpaceOnUse">
           <stop offset="0" stop-color="#FFE9CF" stop-opacity=".07"/><stop offset="1" stop-color="#FFE9CF" stop-opacity="0"/></radialGradient>
           ${g.defs}</defs>
         <rect width="108" height="108" fill="url(#${key}-b)"/><rect width="108" height="108" fill="url(#${key}-h)"/>${g.body}`,
    fg: `<defs>${sh.defs}</defs>${sh.body}${emblemSvg(`stroke="${mark}"`)}`,
  };
}

/** Solid: one flat colour per accent the app offers, so the icon can match the chosen accent. */
function solid(key, label, bgc, mark) {
  const g = grain(`${key}-g`, 0.03);
  return {
    key, family: 'Solid', label,
    bg: `<defs><linearGradient id="${key}-b" x1="0" y1="0" x2="0" y2="1">
           <stop offset="0" stop-color="#fff" stop-opacity=".07"/><stop offset=".55" stop-color="#fff" stop-opacity="0"/>
           <stop offset="1" stop-color="#000" stop-opacity=".10"/></linearGradient>${g.defs}</defs>
         <rect width="108" height="108" fill="${bgc}"/><rect width="108" height="108" fill="url(#${key}-b)"/>${g.body}`,
    fg: emblemSvg(`stroke="${mark}"`),
  };
}

/** Metal: the mark struck in polished metal with a bevel, on a brushed plate of the same metal. */
function metal(key, label, { plate, stops, hi, lo }) {
  const g = `${key}`;
  const stopTags = stops.map((c, i) => `<stop offset="${(i / (stops.length - 1)).toFixed(2)}" stop-color="${c}"/>`).join('');
  return {
    key, family: 'Metal', label,
    bg: `<defs>
           <linearGradient id="${g}-p" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${plate[1]}"/><stop offset="1" stop-color="${plate[0]}"/></linearGradient>
           <filter id="${g}-brush" ${FULL}><feTurbulence type="fractalNoise" baseFrequency="0.003 0.7" numOctaves="2" seed="7"/>
             <feColorMatrix values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 .22 -.07"/></filter>
           <radialGradient id="${g}-sheen" cx="40" cy="26" r="70" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="${hi}" stop-opacity=".22"/><stop offset="1" stop-color="${hi}" stop-opacity="0"/></radialGradient>
         </defs>
         <rect width="108" height="108" fill="url(#${g}-p)"/>
         <rect width="108" height="108" filter="url(#${g}-brush)"/>
         <rect width="108" height="108" fill="url(#${g}-sheen)"/>`,
    fg: `<defs>
           ${markMask(`${g}-m`)}
           ${bevelMask(`${g}-hi`, -0.55, -0.75)}${bevelMask(`${g}-lo`, 0.55, 0.75)}
           <linearGradient id="${g}-f" x1="30" y1="30" x2="80" y2="80" gradientUnits="userSpaceOnUse">${stopTags}</linearGradient>
           <filter id="${g}-sh" ${FULL}><feGaussianBlur stdDeviation="1.4"/><feOffset dy="1.6"/>
             <feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 .7 0"/></filter>
         </defs>
         ${emblemSvg(`stroke="#000" filter="url(#${g}-sh)"`)}
         <rect width="108" height="108" fill="url(#${g}-f)" mask="url(#${g}-m)"/>
         <rect width="108" height="108" fill="${hi}" opacity=".85" mask="url(#${g}-hi)"/>
         <rect width="108" height="108" fill="${lo}" opacity=".7" mask="url(#${g}-lo)"/>`,
  };
}

/** Stealth: a hollow mark drawn in one neon line over a dark instrument grid. */
function stealth(key, label, c, deep) {
  const inner = 4.6 - 1.3;
  return {
    key, family: 'Stealth', label,
    bg: `<defs>
           <pattern id="${key}-grid" width="6" height="6" patternUnits="userSpaceOnUse" x="0" y="0">
             <path d="M6 0 L0 0 0 6" fill="none" stroke="${c}" stroke-width=".18" stroke-opacity=".22"/></pattern>
           <radialGradient id="${key}-v" cx="54" cy="54" r="62" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="${deep}" stop-opacity="1"/><stop offset="1" stop-color="#030304" stop-opacity="1"/></radialGradient>
           <radialGradient id="${key}-fade" cx="54" cy="54" r="54" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient>
           <mask id="${key}-gm"><rect width="108" height="108" fill="url(#${key}-fade)"/></mask>
         </defs>
         <rect width="108" height="108" fill="url(#${key}-v)"/>
         <rect width="108" height="108" fill="url(#${key}-grid)" mask="url(#${key}-gm)"/>`,
    fg: `<defs>
           <mask id="${key}-ring" maskUnits="userSpaceOnUse" x="0" y="0" width="108" height="108">
             ${emblemSvg('stroke="#fff"')}${emblemSvg('stroke="#000"', { strokeOverride: inner })}</mask>
           <filter id="${key}-glow" ${FULL}><feGaussianBlur stdDeviation="2.2"/></filter>
         </defs>
         <g filter="url(#${key}-glow)" opacity=".85"><rect width="108" height="108" fill="${c}" mask="url(#${key}-ring)"/></g>
         <rect width="108" height="108" fill="${c}" mask="url(#${key}-ring)"/>
         <g mask="url(#${key}-ring)"><rect width="108" height="108" fill="#fff" opacity=".35"/></g>`,
  };
}

/** Molten: the mark glowing white-hot from inside, cooling to the family colour at its edges. */
function molten(key, label, [deep, mid, bright]) {
  return {
    key, family: 'Molten', label,
    bg: `<defs>
           <radialGradient id="${key}-glow" cx="54" cy="64" r="58" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="${mid}" stop-opacity=".42"/><stop offset=".45" stop-color="${deep}" stop-opacity=".55"/>
             <stop offset="1" stop-color="#050303"/></radialGradient>
           <filter id="${key}-crack" ${FULL}><feTurbulence type="turbulence" baseFrequency=".045" numOctaves="3" seed="11"/>
             <feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  -3 0 0 0 .3"/>
             <feComposite operator="in" in="SourceGraphic"/><feGaussianBlur stdDeviation=".3"/></filter>
         </defs>
         <rect width="108" height="108" fill="#070403"/>
         <rect width="108" height="108" fill="url(#${key}-glow)"/>
         <rect width="108" height="108" fill="${mid}" filter="url(#${key}-crack)" opacity=".28"/>`,
    fg: `<defs>
           ${markMask(`${key}-m`)}
           <linearGradient id="${key}-f" x1="0" y1="36" x2="0" y2="76" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="${bright}"/><stop offset=".55" stop-color="${mid}"/><stop offset="1" stop-color="${deep}"/></linearGradient>
           <filter id="${key}-bloom" ${FULL}><feGaussianBlur stdDeviation="3.2"/></filter>
           <filter id="${key}-core" ${FULL}><feGaussianBlur stdDeviation=".7"/></filter>
         </defs>
         <g filter="url(#${key}-bloom)" opacity=".9">${emblemSvg(`stroke="${mid}"`)}</g>
         ${emblemSvg(`stroke="url(#${key}-f)"`)}
         <g mask="url(#${key}-m)" filter="url(#${key}-core)">${emblemSvg('stroke="#FFF6E0" stroke-opacity=".9"', { strokeOverride: 1.8 })}</g>`,
  };
}

/** Nebula: bone mark hung in a coloured gas cloud with stars. */
function nebula(key, label, [deep, mid, bright], seed) {
  const sh = liftShadow(`${key}-s`, 0.55, 2, 0.8);
  return {
    key, family: 'Nebula', label,
    bg: `<defs>
           <filter id="${key}-cloud" ${FULL}><feTurbulence type="fractalNoise" baseFrequency=".022" numOctaves="4" seed="${seed}"/>
             <feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  1 0 0 0 0"/>
             <feComponentTransfer><feFuncA type="table" tableValues="0 0 0 .55 1"/></feComponentTransfer>
             <feComposite operator="in" in="SourceGraphic"/></filter>
           <filter id="${key}-cloud2" ${FULL}><feTurbulence type="fractalNoise" baseFrequency=".045" numOctaves="3" seed="${seed + 5}"/>
             <feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  1 0 0 0 0"/>
             <feComponentTransfer><feFuncA type="table" tableValues="0 0 0 .5 .9"/></feComponentTransfer>
             <feComposite operator="in" in="SourceGraphic"/></filter>
           <radialGradient id="${key}-core" cx="60" cy="48" r="52" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="${mid}" stop-opacity=".55"/><stop offset="1" stop-color="${mid}" stop-opacity="0"/></radialGradient>
         </defs>
         <rect width="108" height="108" fill="#07060B"/>
         <rect width="108" height="108" fill="${deep}" filter="url(#${key}-cloud)" opacity=".9"/>
         <rect width="108" height="108" fill="url(#${key}-core)"/>
         <rect width="108" height="108" fill="${mid}" filter="url(#${key}-cloud2)" opacity=".55"/>
         <rect width="108" height="108" fill="${bright}" filter="url(#${key}-cloud2)" opacity=".12"/>
         ${stars(seed, 70, '#fff', 0.5)}`,
    fg: `<defs>${sh.defs}<filter id="${key}-halo" ${FULL}><feGaussianBlur stdDeviation="2.4"/></filter></defs>
         ${sh.body}<g filter="url(#${key}-halo)" opacity=".55">${emblemSvg(`stroke="${bright}"`)}</g>${emblemSvg('stroke="#FBF8F3"')}`,
  };
}

/** Aurora: bone mark under curtains of northern light rising off a dark horizon. */
function aurora(key, label, { sky, ribbons, horizon }, seed) {
  const r = rng(seed);
  let curtains = '';
  ribbons.forEach((c, i) => {
    const y0 = 34 + i * 10 + r() * 6, amp = 8 + r() * 6, ph = r() * 40;
    const pts = [];
    for (let x = -10; x <= 118; x += 8) pts.push([x, y0 + Math.sin((x + ph) / 17) * amp]);
    const d = pts.map((p, j) => `${j ? 'L' : 'M'}${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join(' ');
    curtains += `<path d="${d}" fill="none" stroke="url(#${key}-rib${i})" stroke-width="${22 - i * 4}" filter="url(#${key}-soft)" opacity="${0.85 - i * 0.15}"/>`;
  });
  const ribDefs = ribbons.map((c, i) => `<linearGradient id="${key}-rib${i}" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="${c}" stop-opacity="0"/><stop offset=".6" stop-color="${c}" stop-opacity=".8"/>
      <stop offset="1" stop-color="${c}" stop-opacity=".05"/></linearGradient>`).join('');
  const sh = liftShadow(`${key}-s`, 0.5, 2, 1);
  return {
    key, family: 'Aurora', label,
    bg: `<defs>
           <linearGradient id="${key}-sky" x1="0" y1="0" x2="0" y2="1">
             <stop offset="0" stop-color="${sky[0]}"/><stop offset=".7" stop-color="${sky[1]}"/><stop offset="1" stop-color="${horizon}"/></linearGradient>
           <filter id="${key}-soft" ${FULL}><feGaussianBlur stdDeviation="3.4 5"/></filter>
           ${ribDefs}
         </defs>
         <rect width="108" height="108" fill="url(#${key}-sky)"/>
         ${stars(seed + 1, 40, '#fff', 0.4)}
         <g style="mix-blend-mode:screen">${curtains}</g>`,
    fg: `<defs>${sh.defs}</defs>${sh.body}${emblemSvg('stroke="#FBF8F3"')}`,
  };
}

/** Gem: the mark cut into facets that catch the light unevenly, with one glint. */
function gem(key, label, { tones, glow, holo }, seed) {
  const r = rng(seed);
  // A low-poly shard field across the mark's area; each triangle takes one tone.
  let shards = '';
  const step = 6;
  for (let y = 24; y < 84; y += step) {
    for (let x = 22; x < 88; x += step) {
      const j = () => (r() - 0.5) * 2.4;
      const a = [x + j(), y + j()], b = [x + step + j(), y + j()], c = [x + j(), y + step + j()], d = [x + step + j(), y + step + j()];
      const t1 = tones[Math.floor(r() * tones.length)], t2 = tones[Math.floor(r() * tones.length)];
      const tri = (p, q, s, col) => `<path d="M${p.map((n) => n.toFixed(2)).join(' ')} L${q.map((n) => n.toFixed(2)).join(' ')} L${s.map((n) => n.toFixed(2)).join(' ')}Z" fill="${col}" stroke="${col}" stroke-width=".25"/>`;
      shards += tri(a, b, c, t1) + tri(b, d, c, t2);
    }
  }
  const sparkle = (x, y, s) => `<path d="M${x} ${y - s} L${x + s * 0.18} ${y - s * 0.18} L${x + s} ${y} L${x + s * 0.18} ${y + s * 0.18} L${x} ${y + s} L${x - s * 0.18} ${y + s * 0.18} L${x - s} ${y} L${x - s * 0.18} ${y - s * 0.18}Z" fill="#fff"/>`;
  return {
    key, family: 'Gem', label,
    bg: `<defs><radialGradient id="${key}-g" cx="54" cy="52" r="64" gradientUnits="userSpaceOnUse">
           <stop offset="0" stop-color="${glow}" stop-opacity=".32"/><stop offset="1" stop-color="${glow}" stop-opacity="0"/></radialGradient></defs>
         <rect width="108" height="108" fill="#06070A"/><rect width="108" height="108" fill="url(#${key}-g)"/>
         ${stars(seed + 9, 18, '#fff', 0.35)}`,
    fg: `<defs>${markMask(`${key}-m`)}
           <filter id="${key}-halo" ${FULL}><feGaussianBlur stdDeviation="2.6"/></filter>
           ${holo ? `<linearGradient id="${key}-iri" x1="30" y1="30" x2="78" y2="78" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="#7FE3FF"/><stop offset=".35" stop-color="#B98CFF"/><stop offset=".7" stop-color="#FF8FCF"/><stop offset="1" stop-color="#FFE39A"/></linearGradient>` : ''}
           <linearGradient id="${key}-edge" x1="0" y1="30" x2="0" y2="78" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="#fff" stop-opacity=".55"/><stop offset=".5" stop-color="#fff" stop-opacity="0"/></linearGradient>
         </defs>
         <g filter="url(#${key}-halo)" opacity=".35">${emblemSvg(`stroke="${glow}"`)}</g>
         <g mask="url(#${key}-m)">${shards}
           ${holo ? `<rect width="108" height="108" fill="url(#${key}-iri)" style="mix-blend-mode:color" opacity=".9"/>` : ''}
           <rect width="108" height="108" fill="url(#${key}-edge)"/></g>
         ${sparkle(64.5, 40.5, 4.2)}${sparkle(38.6, 67, 2.2)}`,
  };
}

/** Gym: the mark as the gym itself wears it, in chalk on slate or painted on an iron plate. */
function chalk(key, label) {
  const g = grain(`${key}-g`, 0.05);
  return {
    key, family: 'Gym', label,
    bg: `<defs>
           <filter id="${key}-dust" ${FULL}><feTurbulence type="fractalNoise" baseFrequency=".035" numOctaves="4" seed="21"/>
             <feColorMatrix values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 1.1 -.55"/><feGaussianBlur stdDeviation="1.2"/></filter>
           <radialGradient id="${key}-v" cx="54" cy="50" r="70" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="#2E3337"/><stop offset="1" stop-color="#16191C"/></radialGradient>
           ${g.defs}
         </defs>
         <rect width="108" height="108" fill="url(#${key}-v)"/>
         <rect width="108" height="108" filter="url(#${key}-dust)" opacity=".16"/>
         ${g.body}`,
    fg: `<defs>
           <filter id="${key}-rough" ${FULL}>
             <feTurbulence type="fractalNoise" baseFrequency=".9" numOctaves="2" seed="4" result="n"/>
             <feDisplacementMap in="SourceGraphic" in2="n" scale="1.8" xChannelSelector="R" yChannelSelector="G" result="d"/>
             <feTurbulence type="fractalNoise" baseFrequency=".7" numOctaves="3" seed="9" result="n2"/>
             <feColorMatrix in="n2" values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  -9 0 0 0 6.5" result="holes"/>
             <feComposite in="d" in2="holes" operator="in"/></filter>
           <filter id="${key}-smear" ${FULL}><feGaussianBlur stdDeviation="1.6"/></filter>
         </defs>
         <g filter="url(#${key}-smear)" opacity=".18">${emblemSvg('stroke="#F4F1EA"')}</g>
         <g filter="url(#${key}-rough)">${emblemSvg('stroke="#F4F1EA"')}</g>`,
  };
}

function iron(key, label) {
  const rings = [47, 43.5, 30].map((rad, i) => `
    <circle cx="54" cy="54" r="${rad}" fill="none" stroke="#000" stroke-opacity=".55" stroke-width="${i === 2 ? 0.8 : 1.1}" transform="translate(.35 .5)"/>
    <circle cx="54" cy="54" r="${rad}" fill="none" stroke="#fff" stroke-opacity=".10" stroke-width="${i === 2 ? 0.6 : 0.9}"/>`).join('');
  return {
    key, family: 'Gym', label,
    bg: `<defs>
           <radialGradient id="${key}-p" cx="44" cy="36" r="80" gradientUnits="userSpaceOnUse">
             <stop offset="0" stop-color="#3A3B3E"/><stop offset=".6" stop-color="#202124"/><stop offset="1" stop-color="#121315"/></radialGradient>
           <filter id="${key}-cast" ${FULL}><feTurbulence type="fractalNoise" baseFrequency=".6" numOctaves="3" seed="13"/>
             <feColorMatrix values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 .7 -.3"/></filter>
           <filter id="${key}-pit" ${FULL}><feTurbulence type="fractalNoise" baseFrequency=".5" numOctaves="2" seed="14"/>
             <feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 .9 -.4"/></filter>
         </defs>
         <rect width="108" height="108" fill="url(#${key}-p)"/>
         <rect width="108" height="108" filter="url(#${key}-cast)" opacity=".25"/>
         <rect width="108" height="108" filter="url(#${key}-pit)" opacity=".5"/>
         ${rings}`,
    fg: `<defs>
           ${markMask(`${key}-m`)}
           <filter id="${key}-wear" ${FULL}><feTurbulence type="fractalNoise" baseFrequency=".45" numOctaves="3" seed="17"/>
             <feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  -10 0 0 0 7.1"/><feComposite operator="in" in="SourceGraphic"/></filter>
           <filter id="${key}-sh" ${FULL}><feGaussianBlur stdDeviation=".9"/><feOffset dx=".4" dy=".9"/>
             <feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 .8 0"/></filter>
         </defs>
         ${emblemSvg('stroke="#000" filter="url(#' + key + '-sh)"')}
         ${emblemSvg('stroke="#2A2B2E"')}
         <g transform="translate(-.35 -.45)" mask="url(#${key}-m)">${emblemSvg('stroke="#6B6D72" stroke-opacity=".7"')}</g>
         <g filter="url(#${key}-wear)">${emblemSvg('stroke="#EDEAE3"', { strokeOverride: 3.7 })}</g>`,
  };
}

// ── the set ────────────────────────────────────────────────────────────────────────────────────
// Declaration order is picker order. Keep it in step with the AppIcon enum.

export const ICONS = [
  avex('Default', 'Pearl', '#F2EFEA'),
  avex('AvexSignal', 'Signal', '#E23D3D'),

  solid('SolidRed', 'Red', '#D93636', '#FBF8F3'),
  solid('SolidEmber', 'Ember', '#D4761F', '#1A1210'),
  solid('SolidGold', 'Gold', '#8C7340', '#FBF8F3'),
  solid('SolidOlive', 'Olive', '#4D6040', '#F2EFEA'),
  solid('SolidNavy', 'Navy', '#3D4F73', '#F2EFEA'),
  solid('SolidPaper', 'Paper', '#F4F1EA', '#17120E'),

  metal('MetalGold', 'Gold', { plate: ['#141009', '#2A2215'], stops: ['#7A5A1E', '#F7E4A6', '#B8872F', '#FFF4CC', '#8C6624'], hi: '#FFF4CC', lo: '#3A2A0C' }),
  metal('MetalChrome', 'Chrome', { plate: ['#0E1013', '#252A31'], stops: ['#59616D', '#F4F7FA', '#8A94A2', '#FFFFFF', '#66707C'], hi: '#FFFFFF', lo: '#1C2027' }),
  metal('MetalCopper', 'Copper', { plate: ['#130C09', '#2A1C15'], stops: ['#6E3A1F', '#F2B48A', '#A85A32', '#FFD9BC', '#7C4225'], hi: '#FFE2CC', lo: '#2E170C' }),

  stealth('StealthCrimson', 'Crimson', '#FF4D63', '#1A080C'),
  stealth('StealthCyan', 'Cyan', '#44D4EC', '#06161A'),

  molten('MoltenEmber', 'Ember', ['#5A1E08', '#F07A1E', '#FFE6A8']),
  molten('MoltenPlasma', 'Plasma', ['#3A1760', '#B35CFF', '#F4DCFF']),

  nebula('NebulaViolet', 'Violet', ['#4A1C7A', '#8A3FD0', '#F2E6FF'], 5),
  nebula('NebulaTeal', 'Teal', ['#0E4A4E', '#1FA8A2', '#E2FFFB'], 8),

  aurora('AuroraNorthern', 'Northern', { sky: ['#030A10', '#0A2328'], ribbons: ['#2FD08E', '#7CF0B4', '#3AA7C9'], horizon: '#12343A' }, 3),
  aurora('AuroraDusk', 'Dusk', { sky: ['#140A22', '#4A2358'], ribbons: ['#E0609E', '#F29A5A', '#9A6BE0'], horizon: '#C8663A' }, 12),

  gem('GemEmerald', 'Emerald', { tones: ['#0B4A2E', '#157A4C', '#2FB877', '#6FE0A8', '#B9F5D6', '#0F5E3A'], glow: '#2FB877' }, 31),
  gem('GemHolo', 'Holo', { tones: ['#8FE8FF', '#B79BFF', '#FF9AD5', '#FFE6A8', '#6E7BFF', '#E6D9FF', '#5B3FA0', '#C47BFF'], glow: '#A070F0' }, 37),

  chalk('GymChalk', 'Chalk'),
  iron('GymIron', 'Iron'),
];
