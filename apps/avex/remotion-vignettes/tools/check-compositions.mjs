// Check that render.sh and Root.tsx still agree about which vignettes exist.
//
//   node tools/check-compositions.mjs
//
// These three WebPs are rendered by hand and committed into the Android app's res/raw, so nothing
// downstream notices a rename until an onboarding card is silently missing its animation. Renaming
// a <Composition id> without editing render.sh's loop (or the reverse) is the way that happens, and
// it is exactly the kind of drift no compiler and no test in either project can see: one is a JSX
// attribute and the other is a bash word.
//
// Deliberately dependency-free — this package has no TypeScript of its own, and a check that needs
// an install is a check that does not run.
import {readFileSync} from 'node:fs';

const read = (rel) => readFileSync(new URL(rel, import.meta.url), 'utf8');

const declared = [...read('../src/Root.tsx').matchAll(/<Composition\s[^>]*?id="([^"]+)"/gs)].map((m) => m[1]);
const shell = read('../render.sh');
const rendered = (/for comp in ([^;]+); do/.exec(shell)?.[1] ?? '').trim().split(/\s+/).filter(Boolean);

const problems = [];
if (declared.length === 0) problems.push('No <Composition id="…"> found in src/Root.tsx.');
if (rendered.length === 0) problems.push("Could not read render.sh's composition list.");

for (const id of declared) {
  if (!rendered.includes(id)) problems.push(`src/Root.tsx declares "${id}", which render.sh all never renders.`);
}
for (const id of rendered) {
  if (!declared.includes(id)) problems.push(`render.sh renders "${id}", which src/Root.tsx does not declare.`);
  // The raw resource each one lands on, named by render.sh's own interpolation.
  const raw = `planmode_${id}.webp`;
  try {
    readFileSync(new URL(`../../forge-android/app/src/main/res/raw/${raw}`, import.meta.url));
  } catch {
    problems.push(`res/raw/${raw} is missing — the onboarding card for "${id}" has no animation.`);
  }
}

if (problems.length) {
  console.error(problems.map((p) => `  ✗ ${p}`).join('\n'));
  process.exit(1);
}
console.log(`${declared.length} compositions, each rendered and each present in res/raw: ${declared.join(', ')}`);
