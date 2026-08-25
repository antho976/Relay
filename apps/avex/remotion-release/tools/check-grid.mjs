// Prints every handover against the bar it was declared on. Run after any change to the cut:
//   node tools/check-grid.mjs
// A hard cut must land on its downbeat exactly; a transition's midpoint may sit half a frame past
// it when its length is odd. Anything else means the layout arithmetic is wrong, not the music.
import {readFileSync} from 'node:fs';
const FPS = 30;
const theme = readFileSync(new URL('../src/theme.ts', import.meta.url), 'utf8');
const bed = /firstDownbeat:\s*([\d.]+),\s*bar:\s*([\d.]+)/.exec(theme);
const XF = Object.fromEntries([...theme.matchAll(/(cut|quick|soft|pan|wide):\s*(\d+)/g)].map((m) => [m[1], +m[2]]));
const bar = (n) => Math.round((+bed[1] + (n - 1) * +bed[2]) * FPS);
const overlap = (t) => t === 'cut' ? 0 : /whip/.test(t) ? XF.quick : /push/.test(t) ? XF.pan : /wipe|dissolve/.test(t) ? XF.wide : XF.soft;
const src = readFileSync(new URL('../src/Release.tsx', import.meta.url), 'utf8');
const body = src.slice(src.indexOf('const BEATS'), src.indexOf('/* ── laying'));
const beats = [...body.matchAll(/endBar:\s*(\d+)(?:,\s*out:\s*'(\w+)')?/g)].map((m) => ({endBar: +m[1], out: m[2] ?? (m.index && 'fade')}));
let start = 0, bad = 0;
const rows = [];
for (let i = 0; i < beats.length; i++) {
  const b = beats[i];
  const t = i === beats.length - 1 ? 0 : overlap(b.out ?? 'fade');
  const len = bar(b.endBar) + Math.ceil(t / 2) - start;
  const next = bar(b.endBar) - Math.floor(t / 2);
  const mid = next + t / 2;
  const err = mid - bar(b.endBar);
  if (i < beats.length - 1) rows.push(`bar ${String(b.endBar).padStart(2)}  frame ${String(bar(b.endBar)).padStart(4)}  ${(b.out ?? 'fade').padEnd(8)} T=${String(t).padStart(2)}  beat ${i} = [${start}, ${start + len})  handover mid ${mid}  err ${err > 0 ? '+' : ''}${err}`);
  if (Math.abs(err) > 0.5) bad++;
  start = next;
}
console.log(rows.join('\n'));
console.log(`total ${bar(beats[beats.length - 1].endBar)} frames = ${(bar(beats[beats.length - 1].endBar) / FPS).toFixed(2)} s; ${bad} handover(s) off the grid`);
process.exit(bad ? 1 : 0);
