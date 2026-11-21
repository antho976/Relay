// Prune the by-priority "Ideas at a Glance" file: remove bullets whose bold title matches a
// confirmed removal, drop now-empty priority subheaders, then recompute every count (per-category
// <sub> lines, the summary table, the header total) so the file stays honest + readable.
//   node _prune_bypri.cjs <file.md> <removals.json>
const fs = require('fs');
const [, , filePath, removalsPath] = process.argv;
const removals = JSON.parse(fs.readFileSync(removalsPath, 'utf8')); // [{category,title,verdict,reason}]
const titles = new Set(removals.map((r) => r.title.trim()));
let lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/);

const isBullet = (l) => /^- /.test(l);
const isSub = (l) => /^\*\*P[0-3] · /.test(l);
const isCat = (l) => /^## /.test(l);
const bulletTitle = (l) => { const m = l.match(/\*\*(.+?)\*\*/); return m ? m[1].trim() : null; };

// ── 1. remove matching bullet lines ───────────────────────────────────────────
const matched = new Set();
lines = lines.filter((l) => {
  if (isBullet(l)) { const t = bulletTitle(l); if (t && titles.has(t)) { matched.add(t); return false; } }
  return true;
});

// ── 2. drop priority subheaders that now have no bullets under them ────────────
const drop = new Array(lines.length).fill(false);
for (let i = 0; i < lines.length; i++) {
  if (!isSub(lines[i])) continue;
  let hasBullet = false;
  for (let j = i + 1; j < lines.length; j++) {
    if (isBullet(lines[j])) { hasBullet = true; break; }
    if (isSub(lines[j]) || isCat(lines[j])) break;
  }
  if (!hasBullet) { drop[i] = true; if (i + 1 < lines.length && lines[i + 1].trim() === '') drop[i + 1] = true; }
}
lines = lines.filter((_, i) => !drop[i]);

// ── 3. recompute per-category counts; remember the <sub> line index ────────────
const stats = {}; // num -> {ideas,p0,quick,subIdx}
let curCat = null, curPri = null;
for (let i = 0; i < lines.length; i++) {
  const l = lines[i];
  const cm = l.match(/^## (\d+)\. /);
  if (cm) { curCat = cm[1]; curPri = null; stats[curCat] = { ideas: 0, p0: 0, quick: 0, subIdx: -1 }; continue; }
  if (isSub(l)) { curPri = l.match(/^\*\*(P[0-3])/)[1]; continue; }
  if (/^<sub>/.test(l) && curCat) { stats[curCat].subIdx = i; continue; }
  if (isBullet(l) && curCat) { const s = stats[curCat]; s.ideas++; if (curPri === 'P0') s.p0++; if (l.includes('⚡')) s.quick++; }
}
for (const num in stats) { const s = stats[num]; if (s.subIdx >= 0) lines[s.subIdx] = `<sub>${s.ideas} ideas · ${s.p0} P0 · ${s.quick} quick wins</sub>`; }

// ── 4. rewrite the summary-table rows ─────────────────────────────────────────
for (let i = 0; i < lines.length; i++) {
  const tm = lines[i].match(/^\| (\d+) \| (\[[^\]]+\]\([^)]+\)) \| \d+ \| \d* \| \d* \|/);
  if (tm) { const s = stats[tm[1]]; if (s) lines[i] = `| ${tm[1]} | ${tm[2]} | ${s.ideas} | ${s.p0 || ''} | ${s.quick || ''} |`; }
}

// ── 5. header totals + intro ──────────────────────────────────────────────────
const tot = Object.values(stats).reduce((a, s) => ({ ideas: a.ideas + s.ideas, p0: a.p0 + s.p0, quick: a.quick + s.quick }), { ideas: 0, p0: 0, quick: 0 });
const removed = matched.size;
lines = lines.map((l) =>
  l.replace(/^\*\*\d+ ideas · \d+ P0 · \d+ quick wins\*\*$/, `**${tot.ideas} open ideas · ${tot.p0} P0 · ${tot.quick} quick wins**  ·  pruned ${removed} done/duplicate/not-needed on 2026-06-16 (was 649)`)
   .replace(/^_\d+ ideas, one line each/, `_${tot.ideas} open ideas, one line each`)
);

fs.writeFileSync(filePath, lines.join('\n'), 'utf8');
const unmatched = [...titles].filter((t) => !matched.has(t));
console.log(JSON.stringify({ confirmedTitles: titles.size, removedBullets: removed, keptIdeas: tot.ideas, totals: tot, unmatchedCount: unmatched.length, unmatched }, null, 2));
