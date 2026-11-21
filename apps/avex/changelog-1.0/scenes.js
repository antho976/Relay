
/* ═══ Scenes ═══════════════════════════════════════════════════════════════════════════════════
 * Every item is the final state at HEAD against 8daeb27 (the last commit before 2026-08-26),
 * deduplicated and checked in the code. See README.md for the evidence.
 */
const SCENES = [];
function scene(from, to, build, update, opts = {}) {
  const root = document.createElement('div');
  root.className = 'scene';
  const cam = document.createElement('div');
  cam.className = 'cam';
  root.appendChild(cam);
  stage.appendChild(root);
  const sc = { from: bar(from), to: bar(to), root, cam, zoom: opts.zoom ?? [1, 1.03] };
  sc.refs = build(cam, sc) || {};
  sc.update = update;
  SCENES.push(sc);
  return sc;
}
const PUSH_AT = [5, 7, 9, 11, 13].map(n => bar(n));
const PUSH_LEN = .36;
const press = (t, at) => Math.abs(t - at) < .1 ? 1 - .07 * Math.cos(Math.PI * (t - at) / .2) : 1;
const stepr = (v, size = 22, id = '') => `<span class="stp2"><i>−</i><span class="uh tab" ${id ? `id="${id}"` : ''} style="font-size:${size}px;min-width:${size * 1.6}px;text-align:center">${v}</span><i>+</i></span>`;
const ICON = {
  bar: '<path d="M1 15 H29 M5 8 V22 M8.5 10 V20 M25 8 V22 M21.5 10 V20"/>',
  db: '<path d="M3 15 H27 M6 10 V20 M9 7 V23 M21 7 V23 M24 10 V20"/>',
  cable: '<circle cx="15" cy="5" r="3"/><path d="M15 8 V20 M9 20 H21 M11 20 L10 25 M19 20 L20 25"/>',
};
const glyph = ic => `<div class="glyph"><svg viewBox="0 0 30 30" fill="none" stroke="#F2EFEA" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">${ICON[ic]}</svg></div>`;
const THUMB = {
  reps: '<path d="M8 50 V20 M22 50 V26 M36 50 V33 M50 50 V40"/><path d="M64 50 V44" stroke="#E23D3D"/>',
  volume: '<path d="M6 50 C 20 20, 40 12, 76 10"/><rect x="28" y="6" width="22" height="46" fill="#E23D3D" fill-opacity=".18" stroke="none"/>',
  rest: '<path d="M6 30 H18 C 26 30, 26 48, 36 48 C 48 48, 50 18, 62 18 H76"/><circle cx="76" cy="18" r="3.5" fill="#E23D3D" stroke="none"/>',
};
const MUSCLE = '<svg viewBox="0 0 32 32" style="position:absolute;left:0;top:0;width:32px;height:32px"><path d="M11 9 H21 L23 23 H9 Z" fill="none" stroke="#BFB6AA" stroke-width="1.6" stroke-linejoin="round"/><path d="M12.5 13 H19.5" stroke="#E23D3D" stroke-width="3" stroke-linecap="round"/></svg>';
const photo = (x, y, w, h, a) => `<div class="abs" style="left:${x}px;top:${y}px;width:${w}px;height:${h}px;border-radius:14px;overflow:hidden;background:linear-gradient(${a}deg,#4b4036,#241e19 72%)">
  <div style="position:absolute;right:-18%;top:-40%;width:70%;height:120%;border-radius:50%;background:rgba(255,214,170,.1)"></div>
  <div style="position:absolute;left:0;right:0;bottom:0;height:55%;background:linear-gradient(0deg,rgba(0,0,0,.55),transparent)"></div>
  <div style="position:absolute;left:12px;bottom:${h > 110 ? 26 : 20}px;width:${w * .55}px;height:7px;border-radius:4px;background:rgba(242,239,234,.7)"></div>
  <div style="position:absolute;left:12px;bottom:12px;width:${w * .35}px;height:5px;border-radius:3px;background:rgba(242,239,234,.35)"></div></div>`;

/* ── Title ─────────────────────────────────────────────────────────────────────────────────────── */
scene(1, 3, cam => {
  cam.innerHTML = `
    <div class="abs" style="left:0;top:0;width:1920px;height:1080px;background:radial-gradient(900px 560px at 50% 44%, rgba(226,61,61,.12), transparent 70%)"></div>
    <div class="abs mono" id="lab" style="left:706px;top:300px;color:var(--ink)">Changelog</div>
    <div class="abs" id="mk" style="left:520px;top:352px;width:160px;height:134px"></div>
    <div class="abs serif" id="word" style="left:700px;top:330px;font-size:190px;letter-spacing:-.04em;font-weight:500;font-variation-settings:'SOFT' 100">${words('Avex')}</div>
    <div class="abs" style="left:1190px;top:360px;width:260px;height:150px;overflow:hidden">
      <div class="abs serif tab" id="v0" style="left:0;top:0;font-size:120px;color:var(--muted)">0.9</div>
      <div class="abs serif tab red" id="v1" style="left:0;top:0;font-size:120px">1.0</div></div>
    <div class="abs mono" id="stats" style="left:706px;top:560px">177 commits · 766 files changed</div>`;
  const mk = cam.querySelector('#mk'); mk.innerHTML = markSVG();
  return { lab: cam.querySelector('#lab'), mk, word: cam.querySelector('#word'), v0: cam.querySelector('#v0'), v1: cam.querySelector('#v1'),
           stats: cam.querySelector('#stats'), nori: mkChar(cam, CH.nori, 230) };
}, (t, r) => {
  drawMark(r.mk, prog(t, bar(1) + .1, 1), E.out(prog(t, bar(1, 2), .3)));
  io(r.word, t, bar(1) + .05, 1e9, .05, .7);
  show(r.lab, E.out(prog(t, bar(1, 1), .4)));
  const k = prog(t, bar(1, 2) - .12, .3);
  r.v0.style.transform = `translateY(${-E.in(clamp(k / .5)) * 150}px)`; show(r.v0, k < .5 ? 1 : 0);
  r.v1.style.transform = `translateY(${(1 - E.out(clamp((k - .5) / .5))) * 150}px)`; show(r.v1, k > .5 ? 1 : 0);
  show(r.stats, E.out(prog(t, bar(2) - LEAD, .4)));
  const s = t - bar(2, 2);
  r.nori.set(960, 1000, s < 0 ? {} : { mood: 'happy', ...Act.hop(s, .4, 20), ...Act.wave(t), blink: Act.blink(t) }, s > -.05 ? 1 : 0);
}, { zoom: [1.04, 1] });

/* ── The six big ones: one phone, 0.9 on the left of the seam, 1.0 on the right ────────────────── */
function feature(k, spec) {
  const B = 3 + 2 * k;
  scene(B, B + 2, cam => {
    cam.innerHTML = `
      <div class="abs" style="left:0;top:0;width:1920px;height:1080px;background:radial-gradient(700px 600px at 34% 52%, rgba(226,61,61,.07), transparent 70%)"></div>
      <div class="phone" style="left:430px;top:62px;transform:scale(1.22);transform-origin:0 0"><div class="scr">
        <div class="layer" id="before">${spec.before}</div>
        <div class="layer" id="after">${spec.after}</div>
        <div class="abs" id="seam" style="left:0;top:0;width:4px;height:752px;background:var(--accent);box-shadow:0 0 22px 5px rgba(226,61,61,.55)"></div>
      </div><div class="isl"></div></div>
      <div class="abs mono" style="left:1040px;top:300px">${String(k + 1).padStart(2, '0')} / 06</div>
      <div class="abs serif" id="ttl" style="left:1034px;top:342px;font-size:76px">${lines([spec.title])}</div>
      <div class="abs sans muted" id="sub" style="left:1040px;top:456px;width:690px;font-size:30px;line-height:1.42">${spec.line}</div>
      <div class="abs" id="vt" style="left:1040px;top:600px;display:flex;align-items:center;gap:14px">
        <div class="vtag" id="v0" style="position:static">0.9</div><span class="mono" style="font-size:20px">→</span><div class="vtag" id="v1" style="position:static">1.0</div></div>`;
    const r = { before: cam.querySelector('#before'), after: cam.querySelector('#after'), seam: cam.querySelector('#seam'),
                v0: cam.querySelector('#v0'), v1: cam.querySelector('#v1'), vt: cam.querySelector('#vt'), ttl: cam.querySelector('#ttl'), sub: cam.querySelector('#sub'),
                pal: mkChar(cam, CH[spec.pal], 200) };
    spec.refs?.(cam, r);
    return r;
  }, (t, r) => {
    const land = bar(B + 1);
    // The seam sweeps across the screen in the beat before bar B+1 and lands on the downbeat.
    const p = E.inOut(prog(t, land - BEAT, BEAT));
    r.after.style.clipPath = `inset(0 ${((1 - p) * 100).toFixed(3)}% 0 0)`;
    r.seam.style.transform = `translateX(${p * 352 - 2}px)`;
    r.seam.style.opacity = p > 0 && p < 1 ? 1 : 0;
    // Which version the phone is showing: 0.9 lit before the seam, 1.0 after it.
    const on = t >= land - .05;
    r.v0.style.cssText = `position:static;border:2px solid ${on ? 'var(--line)' : 'var(--ink)'};color:${on ? 'var(--muted)' : 'var(--ink)'}`;
    r.v1.style.cssText = `position:static;border:2px solid ${on ? 'var(--accent)' : 'var(--line)'};background:${on ? 'var(--accent)' : 'transparent'};color:${on ? '#110F0C' : 'var(--muted)'}`;
    const tk = E.expo(prog(t, bar(B, 1.5) - LEAD, .5));
    r.vt.style.opacity = tk; r.vt.style.transform = `translateY(${(1 - tk) * 14}px) scale(${on ? 1 + .06 * Math.exp(-(t - land) * 8) : 1})`;
    r.vt.style.transformOrigin = '0 50%';
    ioLines(r.ttl, t, [bar(B) - LEAD]);
    const sk = E.expo(prog(t, bar(B, 1) - LEAD, .6));
    show(r.sub, sk); r.sub.style.transform = `translateY(${(1 - sk) * 20}px)`;
    const s = t - land;
    r.pal.set(270, 1000, s < 0 ? { mood: 'calm', ...Act.breathe(t), look: [4, -2], blink: Act.blink(t, k), wig: wig(t), glasses: spec.pal === 'mochi' && k === 1 }
      : s < .45 ? { mood: 'shock', ...Act.hop(s, .4, 24), armR: 110, armL: 110, fx: 'shock', time: t, wig: wig(t, 5, 12), glasses: spec.pal === 'mochi' && k === 1 }
      : { mood: 'cheer', ...Act.cheer(s, 18), fx: 'sparkles', time: t, wig: wig(t, 3, 10), glasses: spec.pal === 'mochi' && k === 1 });
    spec.update?.(t, r, B);
  });
}

/* 01 · Set-by-set logging */
feature(0, {
  pal: 'kettle', title: 'Set-by-set logging', line: 'Each set is already filled in, so a repeat set is one tap.',
  before: `
    <div class="sb">9:41</div>
    <div class="abs" style="left:20px;top:46px;font-size:20px;color:var(--muted)">←</div>
    <div class="abs um" style="left:20px;top:84px">Freestyle · 0:00</div>
    <div class="abs us" style="left:20px;top:104px;width:312px;line-height:1.4">Log what you did — add the machines and exercises, with your weights and reps.</div>
    <div class="abs" style="left:20px;top:160px;width:312px;height:220px">
      <div class="abs" style="left:0;top:0;width:32px;height:32px;border-radius:9px;background:var(--tile)">${MUSCLE}</div>
      <div class="abs ut" style="left:44px;top:0">Bench Press</div>
      <div class="abs um" style="left:44px;top:21px">Chest · 2 sets</div>
      <div class="abs" style="right:0;top:2px;color:var(--muted);font-size:15px">▾ &nbsp;×</div>
      <div class="abs us" style="left:44px;top:40px;color:var(--accent)">copy last time</div>
      <div class="abs um" style="left:0;top:74px">Set</div><div class="abs um" style="left:38px;top:74px">Weight · kg</div><div class="abs um" style="left:172px;top:74px">Reps</div>
      ${[1, 2].map(i => `<div class="abs" style="left:0;top:${94 + (i - 1) * 40}px;width:312px;height:36px;display:flex;align-items:center;gap:6px">
        <span class="um" style="width:26px">${i}</span>${stepr('60', 18)}<span style="width:8px"></span>${stepr('8', 18)}<span class="us" style="margin-left:auto">⋯ &nbsp;×</span></div>`).join('')}
      <div class="abs us" style="left:0;top:180px;color:var(--ink)">+ add set</div>
    </div>
    <div class="abs ut" style="left:20px;top:404px;width:312px;height:46px;border-radius:12px;border:1.5px solid var(--line);display:flex;align-items:center;justify-content:center">+ Add exercise</div>
    <div class="abs us" style="left:20px;top:464px;color:var(--accent)">start from a past workout →</div>
    <div class="abs ut" style="left:20px;right:20px;bottom:30px;height:54px;border-radius:12px;background:var(--tile);display:flex;align-items:center;justify-content:center">Save workout</div>`,
  after: `
    <div class="sb">9:41</div>
    <div class="abs um" style="left:20px;top:50px">Open workout · 12:04 · <span style="color:var(--accent)">Rest 1:32</span></div>
    <div class="abs" style="left:20px;top:72px;width:312px;display:flex;gap:28px">
      ${[['1', 'Exercises', 'fx1'], ['2', 'Sets', 'fs1'], ['960', 'Volume · kg', 'fv1']].map(([n, l, id]) => `<div><div class="uh tab" id="${id}" style="font-size:30px">${n}</div><div class="um" style="margin-top:2px">${l}</div></div>`).join('')}</div>
    <div class="abs uh" style="left:20px;top:140px;font-size:30px">Bench Press</div>
    <div class="abs" style="left:20px;top:184px;width:250px"><span class="um" style="margin-right:6px">Last</span>${['60 × 8', '60 × 8', '60 × 7'].map(c => `<span class="pl" style="color:var(--ink)">${c}</span>`).join('')}</div>
    <div class="abs um" style="right:20px;top:190px;color:var(--accent)">Repeat all →</div>
    ${[['01', '60 × 8', ''], ['02', '60 × 8', '△ Last'], ['03', '60 × 8', '']].map(([n, v, m], i) => `<div class="abs lrow1" style="left:20px;top:${222 + i * 44}px;width:312px;height:40px">
      <span class="abs um" style="left:0;top:14px">${n}</span><span class="abs uh tab" style="left:36px;top:2px;font-size:28px">${v}</span>
      ${m ? `<span class="abs um" style="right:0;top:14px;color:var(--accent)">${m}</span>` : ''}</div>`).join('')}
    <div class="abs" style="left:20px;top:362px;width:312px;height:166px;border-radius:16px;background:var(--tile)">
      <div class="abs um" id="setl" style="left:16px;top:14px;color:var(--ink)">Set 03</div><div class="abs um" style="right:16px;top:14px">Tags ○</div>
      <div class="abs" style="left:16px;top:44px">${stepr('60', 26)}</div><div class="abs" style="left:176px;top:44px">${stepr('8', 26)}</div>
      <div class="abs" id="logb" style="left:16px;right:16px;bottom:16px;height:48px;border-radius:12px;background:var(--accent);color:#110F0C;display:flex;align-items:center;justify-content:center;font-family:var(--sans);font-weight:500;font-size:16px">Log set</div></div>
    <div class="abs us" style="left:20px;top:548px;width:312px;height:40px;border-radius:12px;border:1.5px solid var(--line);display:flex;align-items:center;padding-left:14px;box-sizing:border-box">Add an exercise</div>
    <div class="abs um" style="left:20px;top:604px">Recent</div>
    <div class="abs" style="left:20px;top:624px;display:flex;gap:8px">${['Squat', 'Row', 'Press', 'Curl'].map(n => `<span class="us" style="padding:10px 12px;border-radius:10px;background:var(--tile);color:var(--ink)">${n}</span>`).join('')}</div>
    <div class="abs ut" id="save" style="left:20px;right:20px;bottom:24px;height:50px;border-radius:12px;background:var(--ink);color:#110F0C;display:flex;align-items:center;justify-content:center;font-weight:500">Save workout · 2 sets</div>`,
  refs: (cam, r) => Object.assign(r, { rows: [...cam.querySelectorAll('.lrow1')], setl: cam.querySelector('#setl'), logb: cam.querySelector('#logb'),
                                      fs: cam.querySelector('#fs1'), fv: cam.querySelector('#fv1'), save: cam.querySelector('#save') }),
  update: (t, r, B) => {
    const tap = bar(B + 1, 1), done = t >= tap;
    r.logb.style.transform = `scale(${press(t, tap)})`;
    const k = E.expo(prog(t, tap, .4));
    r.rows[2].style.opacity = done ? k : 0; r.rows[2].style.transform = `translateY(${(1 - k) * 14}px)`;
    r.setl.textContent = done ? 'Set 04' : 'Set 03';
    r.fs.textContent = done ? '3' : '2'; r.fv.textContent = done ? '1,440' : '960';
    r.save.textContent = `Save workout · ${done ? 3 : 2} sets`;
  },
});

/* 02 · Academy, redrawn */
const REPS = [8, 8, 9, 9, 10, 10, 8, 8];
feature(1, {
  pal: 'mochi', title: 'Academy, redrawn', line: '35 photo pieces became 12 lessons. Each one is taught by a drawing that draws itself.',
  before: `
    <div class="sb">9:41</div>
    <div class="abs um" style="left:20px;top:52px">35 pieces</div>
    <div class="abs uh" style="left:18px;top:68px;font-size:40px">Academy</div>
    <div class="abs us" style="left:20px;top:118px;font-style:italic">Everything the coach knows, open from the start.</div>
    ${photo(20, 148, 312, 150, 130)}<div class="abs um" style="left:34px;top:162px;color:var(--ink)">Start here</div>
    <div class="abs um" style="left:20px;top:318px">Fundamentals</div>
    ${photo(20, 338, 312, 120, 150)}${photo(20, 468, 152, 100, 110)}${photo(180, 468, 152, 100, 160)}
    <div class="abs um" style="left:20px;top:588px">How the coach works</div>
    ${photo(20, 608, 152, 100, 140)}${photo(180, 608, 152, 100, 120)}`,
  after: (() => {
    const ax = i => 26 + i * 36, ay = v => 138 - (v - 7) * 30;
    let d = `M ${ax(0)} ${ay(REPS[0])}`;
    for (let i = 1; i < REPS.length; i++) d += ` H ${ax(i)} V ${ay(REPS[i])}`;
    d += ` H ${ax(7) + 20}`;
    return `
    <div class="sb">9:41</div>
    <div class="abs uh" style="left:18px;top:50px;font-size:40px">Academy</div>
    <div class="abs us" style="left:20px;top:100px">12 lessons · 60 min of reading</div>
    <svg class="abs" viewBox="0 0 312 160" style="left:20px;top:126px;width:312px;height:160px;overflow:visible">
      ${[8, 9, 10].map(v => `<line x1="26" x2="300" y1="${ay(v)}" y2="${ay(v)}" stroke="#BFB6AA" stroke-opacity=".3" stroke-dasharray="2 5"/>`).join('')}
      <line x1="26" x2="300" y1="150" y2="150" stroke="#BFB6AA" stroke-width="1.5"/><line x1="26" x2="26" y1="150" y2="10" stroke="#BFB6AA" stroke-width="1.5"/>
      <path id="f2ink" d="${d}" fill="none" stroke="#F2EFEA" stroke-width="3" stroke-linejoin="round"/>
      <circle id="f2acc" cx="${ax(6)}" cy="${ay(8)}" r="0" fill="#E23D3D"/>
      <text id="f2plus" x="${ax(6) - 6}" y="${ay(10) - 16}" font-family="Fraunces, serif" font-size="22" fill="#E23D3D" opacity="0">+2.5 kg</text></svg>
    <div class="abs uh" style="left:20px;top:298px;font-size:21px">How you actually get stronger</div>
    <div class="abs us" style="left:20px;top:328px;width:312px;line-height:1.4">Repeat the same lifts and ask for a little more each time. Reps first, then weight.</div>
    <div class="abs um" style="left:20px;top:374px">1 of 4 · 5 min · <span style="color:var(--accent)">read →</span></div>
    <div class="abs uh" style="left:20px;top:408px;font-size:22px">Training</div>
    <div class="abs us" style="left:20px;top:436px">How lifting works. Best read in order.</div>
    ${[['reps', 'How hard a set should be', true], ['volume', 'How much work a muscle needs', false], ['rest', 'Rest is where you grow', false]].map(([th, n, fy], i) => `
      <div class="abs" style="left:20px;top:${468 + i * 74}px;width:312px;height:64px">
        <div class="abs" style="left:0;top:4px;width:72px;height:54px;border-radius:12px;background:var(--tile)"><svg viewBox="0 0 84 58" style="position:absolute;left:8px;top:6px;width:56px;height:40px;overflow:visible" fill="none" stroke="#F2EFEA" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round">${THUMB[th]}</svg></div>
        <div class="abs ut" style="left:86px;top:6px;font-size:14px">${n}</div>
        <div class="abs us" style="left:86px;top:30px">5 min${fy ? ' · <span id="fy" style="color:var(--accent)">● For you</span>' : ''}</div></div>`).join('')}`;
  })(),
  refs: (cam, r) => {
    const ink = cam.querySelector('#f2ink');
    const len = ink.getTotalLength(); ink.style.strokeDasharray = `${len} ${len}`;
    Object.assign(r, { ink, len, acc: cam.querySelector('#f2acc'), plus: cam.querySelector('#f2plus'), fy: cam.querySelector('#fy') });
  },
  update: (t, r, B) => {
    const k = E.inOut(prog(t, bar(B + 1), 1.2));
    r.ink.style.strokeDashoffset = r.len * (1 - k);
    const s = t - (bar(B + 1) + 1.2);
    r.acc.setAttribute('r', s > 0 ? 5 * E.back(clamp(s / .25)) : 0);
    r.plus.setAttribute('opacity', E.out(prog(t, bar(B + 1) + 1.25, .3)));
    r.fy.style.opacity = .55 + .45 * Math.abs(Math.sin(t * 3));
  },
});

/* 03 · A simpler Coach page */
feature(2, {
  pal: 'nori', title: 'A simpler Coach page', line: 'This week’s calls by default. The readings behind them are one switch away.',
  before: `
    <div class="sb">9:41</div>
    <div class="abs" id="scroll3" style="left:0;top:0;width:352px;height:1200px">
      <div class="abs um" style="left:20px;top:56px;color:var(--ink)">This week</div><div class="abs um" style="right:20px;top:56px">Next brief in 3 days</div>
      <div class="abs" style="left:20px;top:78px;width:312px;height:112px;border-radius:14px;background:var(--tile)">
        <div class="abs us" style="left:14px;top:12px">Back Squat</div><div class="abs uh" style="left:14px;top:30px;font-size:22px">3 → 4 sets</div>
        <div class="abs" style="left:14px;top:72px;display:flex;gap:12px;align-items:center"><span class="us" style="background:var(--accent);color:#110F0C;padding:6px 12px;border-radius:999px">Apply</span><span class="us">Skip</span></div></div>
      <div class="abs um" style="left:20px;top:214px;color:var(--ink)">Signals</div>
      <div class="abs ut" style="left:20px;top:236px;font-size:13px">Recovery load</div>
      <div class="abs" style="left:20px;top:260px;width:312px;height:6px;border-radius:3px;background:var(--line)"><div style="width:80%;height:100%;border-radius:3px;background:var(--ink)"></div></div>
      <div class="abs um" style="left:20px;top:274px">Now 48</div><div class="abs um" style="right:20px;top:274px">Deload at 60</div>
      ${[['Bench Press', '↑ 4%'], ['Back Squat', 'stalling'], ['Barbell Row', 'flat']].map(([n, v], i) => `<div class="abs" style="left:20px;top:${302 + i * 30}px;width:312px;height:26px">
        <span class="abs ut" style="left:0;top:4px;font-size:13px">${n}</span>
        <svg class="abs" viewBox="0 0 60 16" style="left:150px;top:4px;width:60px;height:16px"><polyline points="0,12 12,11 24,9 36,9 48,6 60,${i ? 8 : 3}" fill="none" stroke="#BFB6AA" stroke-width="1.6"/></svg>
        <span class="abs um" style="right:0;top:7px">${v}</span></div>`).join('')}
      <div class="abs um" style="left:20px;top:410px;color:var(--ink)">Block</div>
      <div class="abs" style="left:20px;top:432px;width:312px;height:6px;display:flex;gap:4px">${[1, 1, 0, 0].map(o => `<i style="flex:1;border-radius:3px;background:${o ? 'var(--accent)' : 'var(--line)'}"></i>`).join('')}</div>
      <div class="abs" style="left:20px;top:446px;width:312px;display:flex;justify-content:space-between">${['Accumulate', 'Intensify', 'Peak', 'Deload'].map(p => `<span class="um" style="font-size:8px">${p}</span>`).join('')}</div>
      <div class="abs us" style="left:20px;top:466px">Week 2 of 5, building volume. Deload in 3 weeks.</div>
      <div class="abs um" style="left:20px;top:504px;color:var(--ink)">What it reads</div>
      ${['Sleep', 'Resting heart rate'].map((n, i) => `<div class="abs" style="left:20px;top:${526 + i * 30}px;width:312px;height:26px"><span class="abs ut" style="left:0;top:4px;font-size:13px">${n}</span>
        <span class="abs" style="right:0;top:6px;display:flex;gap:3px">${[6, 9, 7, 10, 8, 5, 9].map(h => `<i style="display:block;width:6px;height:${h}px;margin-top:${12 - h}px;background:rgba(242,239,234,.35);border-radius:1px"></i>`).join('')}</span></div>`).join('')}
      <div class="abs um" style="left:20px;top:600px;color:var(--ink)">Learned</div>
      <div class="abs um" style="left:20px;top:620px">Autopilot · 1 of 3 earned</div>
      ${[['Back Squat · 3 → 4 sets', 'worked'], ['Bench Press · 6-8 reps', 'worked'], ['Lateral Raise · 3 → 4', 'didn’t stick']].map(([n, v], i) => `<div class="abs" style="left:20px;top:${644 + i * 32}px;width:312px;height:28px">
        <span class="abs us" style="left:0;top:6px;color:var(--ink)">${n}</span><span class="abs um" style="right:0;top:6px;color:${i < 2 ? 'var(--accent)' : 'var(--muted)'}">${v}</span></div>`).join('')}
      <div class="abs um" style="left:20px;top:760px;color:var(--ink)">Week of Aug 10</div><div class="abs um" style="right:20px;top:760px">2 calls</div>
      <div class="abs um" style="left:20px;top:800px;color:var(--ink)">Next</div><div class="abs um" style="right:20px;top:800px">3 ahead</div>
    </div>`,
  after: `
    <div class="sb">9:41</div>
    <div class="abs um" style="left:20px;top:56px;color:var(--ink)">This week</div><div class="abs um" style="right:20px;top:56px">Next brief in 3 days</div>
    ${[['Back Squat', '3 → 4 sets', 'Legs are climbing slowest · one more set.'], ['Bench Press', 'Move to 6-8 reps', 'Stalled for 4 sessions · a new rep range.']].map(([a, b, c], i) => `
      <div class="abs" style="left:20px;top:${80 + i * 136}px;width:312px;height:124px;border-radius:14px;background:var(--tile)">
        <div class="abs us" style="left:14px;top:12px">${a}</div><div class="abs uh" style="left:14px;top:30px;font-size:22px">${b}</div>
        <div class="abs us" style="left:14px;top:62px">${c}</div>
        <div class="abs" style="left:14px;top:86px;display:flex;gap:12px;align-items:center"><span class="us" style="background:var(--accent);color:#110F0C;padding:6px 12px;border-radius:999px">Apply</span><span class="us">Skip</span></div></div>`).join('')}
    <div class="abs ut" style="left:20px;top:360px;font-size:14px;color:var(--accent);font-weight:500">Apply all 2 →</div>
    <div class="abs um" style="left:20px;top:404px;color:var(--ink)">Week of Sep 14</div><div class="abs um" style="right:20px;top:404px">2 calls</div>
    <div class="abs um" style="left:20px;top:432px;color:var(--ink)">Week of Sep 7</div><div class="abs um" style="right:20px;top:432px">1 call</div>
    <div class="abs um" style="left:20px;top:474px;color:var(--ink)">Next</div><div class="abs um" style="right:20px;top:474px">3 ahead</div>
    <div class="abs" id="card3" style="left:14px;top:48px;width:324px;height:198px;border-radius:18px;background:#2A231E;box-shadow:inset 0 0 0 1.5px #3E352E">
      <div class="abs um" style="left:18px;top:16px">Coach</div>
      <div class="abs ut" style="left:18px;top:34px;font-size:16px;font-weight:500">You can turn on advanced tracking</div>
      <div class="abs us" style="left:18px;top:60px;width:288px;line-height:1.4">It shows the readings behind each call: signals, block, inputs and what has been learned about you.</div>
      <div class="abs" style="left:18px;top:140px;display:flex;gap:14px;align-items:center"><span class="ut" id="ton" style="font-size:14px;background:var(--accent);color:#110F0C;padding:9px 16px;border-radius:999px;font-weight:500">Turn on</span>
        <span class="us" style="color:var(--ink)">Remind me later</span><span class="us">Ignore</span></div></div>`,
  refs: (cam, r) => Object.assign(r, { scroll: cam.querySelector('#scroll3'), card: cam.querySelector('#card3') }),
  update: (t, r, B) => {
    // 0.9: the page is one long column, so it scrolls; 1.0: the prompt slides in from the top.
    r.scroll.style.transform = `translateY(${-300 * E.inOut(prog(t, bar(B) + .2, BAR * .9))}px)`;
    const ck = E.expo(prog(t, bar(B + 1, .5) - LEAD, .55));
    r.card.style.transform = `translateY(${(1 - ck) * -280}px)`;
  },
});

/* 04 · Your program as a week */
const PDAYS = [['Push', 16, ['Bench Press', 'Overhead Press', 'Incline DB Press', 'Lateral Raise', 'Triceps Pushdown'], ['bar', 'bar', 'db', 'db', 'cable']],
               ['Pull', 16, ['Barbell Row', 'Lat Pulldown', 'Seated Cable Row', 'Face Pull', 'Cable Curl'], ['bar', 'cable', 'cable', 'cable', 'cable']],
               ['Legs', 18, ['Back Squat', 'Romanian Deadlift', 'Leg Press', 'Seated Leg Curl', 'Calf Raise'], ['bar', 'bar', 'cable', 'cable', 'db']],
               ['Upper', 14, ['Bench Press', 'Barbell Row', 'Lateral Raise', 'Cable Curl', 'Triceps Pushdown'], ['bar', 'bar', 'db', 'cable', 'cable']]];
const dayRows = (d, id) => `<div class="abs" id="${id}" style="left:20px;top:328px;width:312px;height:240px">${d[2].map((n, i) => `<div class="abs" style="left:0;top:${i * 46}px;width:312px;height:34px">${glyph(d[3][i])}
    <div class="abs ut" style="left:44px;top:7px;font-size:14px">${n}</div><div class="abs um" style="right:0;top:11px">3 × ${d[0] === 'Legs' ? '8-12' : '6-8'}</div></div>`).join('')}</div>`;
feature(3, {
  pal: 'momo', title: 'Your program as a week', line: 'One bar per day. Tap a day to read it, hold a bar to reorder.',
  before: `
    <div class="sb">9:41</div>
    <div class="abs uh" style="left:18px;top:52px;font-size:32px">Your program</div>
    <div class="abs um" style="left:20px;top:98px">4 days · 64 sets</div>
    ${PDAYS.map((d, j) => `<div class="abs" style="left:20px;top:${126 + j * 150}px;width:312px;height:140px">
      <div class="abs" style="left:0;top:6px;width:8px;height:8px;border-radius:50%;background:${['#E23D3D', '#BFB6AA', '#8FA3C7', '#86B05E'][j]}"></div>
      <div class="abs ut" style="left:16px;top:0;font-weight:500">${d[0]}</div><div class="abs um" style="right:0;top:4px">${d[1]} sets</div>
      ${d[2].slice(0, 3).map((n, i) => `<div class="abs us" style="left:16px;top:${28 + i * 26}px;color:var(--ink);font-size:13px">${n}</div><div class="abs um" style="right:0;top:${30 + i * 26}px">3 × 6-8</div>`).join('')}</div>`).join('')}`,
  after: `
    <div class="sb">9:41</div>
    <div class="abs uh" style="left:18px;top:52px;font-size:32px">Your program</div>
    <div class="abs us" style="left:20px;top:96px">Tap a day to read it.</div>
    <div class="abs um" style="left:20px;top:124px">4 days</div><div class="abs um" style="right:20px;top:124px">64 sets</div>
    <div class="abs" id="bars4" style="left:20px;top:146px;width:312px;height:136px">
      ${PDAYS.map((d, i) => `<div class="abs wb" style="left:${i * 83}px;bottom:24px;width:64px;height:${d[1] * 6}px;border-radius:9px 9px 3px 3px"></div>
        <div class="abs um wl" style="left:${i * 83}px;bottom:0;width:64px;text-align:center">${d[0]}</div>`).join('')}</div>
    <div class="abs um" id="hdrA" style="left:20px;top:300px;color:var(--ink)">Push · 5 moves · 16 sets</div>
    <div class="abs um" id="hdrB" style="left:20px;top:300px;color:var(--ink)">Pull · 5 moves · 16 sets</div>
    ${dayRows(PDAYS[0], 'rowsA')}${dayRows(PDAYS[1], 'rowsB')}
    <div class="abs um" style="left:20px;top:574px;color:var(--accent)">Edit day →</div>`,
  refs: (cam, r) => Object.assign(r, { wb: [...cam.querySelectorAll('.wb')], wl: [...cam.querySelectorAll('.wl')], hdrA: cam.querySelector('#hdrA'), hdrB: cam.querySelector('#hdrB'),
                                      rowsA: cam.querySelector('#rowsA'), rowsB: cam.querySelector('#rowsB') }),
  update: (t, r, B) => {
    // Tap Pull on beat 1; hold Upper on beat 2.5 and drag it one slot left.
    const tap = bar(B + 1, 1), hk = prog(t, tap - .1, .3);
    const sel = t >= tap ? 1 : 0;
    const drag = E.inOut(prog(t, bar(B + 1, 2.5), .6)), lifted = t >= bar(B + 1, 2.3) && t < bar(B + 1, 3.3);
    r.wb.forEach((b, i) => {
      let x = i * 83;
      if (i === 3) x = lerp(3 * 83, 2 * 83, drag);
      if (i === 2) x = lerp(2 * 83, 3 * 83, drag);
      b.style.left = `${x}px`; r.wl[i].style.left = `${x}px`;
      b.style.background = i === sel ? 'var(--accent)' : 'rgba(242,239,234,.25)';
      b.style.transform = i === 3 && lifted ? 'translateY(-10px) scale(1.06)' : 'none';
      r.wl[i].style.color = i === sel ? 'var(--ink)' : 'var(--muted)';
    });
    show(r.hdrA, handOut(hk)); show(r.rowsA, handOut(hk));
    show(r.hdrB, handIn(hk)); show(r.rowsB, handIn(hk));
  },
});

/* 05 · Tap a day you trained */
const HEAT = [0, 2, 0, 1, 3, 0, 0, 2, 0, 3, 0, 1, 0, 2, 1, 0, 3, 0, 2, 0, 0, 3, 0, 2, 0, 1, 3, 0, 0, 2, 0, 3, 0, 0, 1];
const heatFill = v => ['#221C16', 'rgba(226,61,61,.32)', 'rgba(226,61,61,.6)', '#E23D3D'][v];
const grid = id => `<div class="abs" id="${id}" style="left:24px;top:86px;width:304px;height:216px">${HEAT.map((v, i) => `<div class="abs sq" style="left:${(i % 7) * 44}px;top:${Math.floor(i / 7) * 44}px;width:40px;height:40px;border-radius:8px;background:${heatFill(v)}"></div>`).join('')}</div>`;
feature(4, {
  pal: 'nori', title: 'Tap a day you trained', line: 'Page back through the months and open what you did on any day.',
  before: `
    <div class="sb">9:41</div>
    <div class="abs um" style="left:20px;top:58px;color:var(--ink)">Activity</div><div class="abs um" style="right:20px;top:58px">August 2026</div>
    ${grid('g0')}
    <div class="abs" style="left:24px;top:316px;display:flex;gap:5px;align-items:center"><span class="um" style="margin-right:4px">Less</span>${[0, 1, 2, 3].map(v => `<i style="display:block;width:12px;height:12px;border-radius:3px;background:${heatFill(v)}"></i>`).join('')}<span class="um" style="margin-left:4px">More</span></div>
    <div class="abs" style="left:24px;top:354px;width:304px;display:flex;justify-content:space-between">${[['14', 'Active days'], ['16', 'Sessions'], ['3', 'Day streak']].map(([n, l]) => `<div><div class="uh tab" style="font-size:30px">${n}</div><div class="um">${l}</div></div>`).join('')}</div>`,
  after: `
    <div class="sb">9:41</div>
    <div class="abs um" style="left:20px;top:58px;color:var(--ink)">Activity</div><div class="abs um" style="right:20px;top:58px;color:var(--ink)">‹ &nbsp;September 2026&nbsp; ›</div>
    ${grid('g1')}
    <div class="abs" id="ring5" style="left:0;top:0;width:44px;height:44px;border-radius:10px;border:3px solid var(--ink)"></div>
    <div class="abs" style="left:24px;top:316px;display:flex;gap:5px;align-items:center"><span class="um" style="margin-right:4px">Less</span>${[0, 1, 2, 3].map(v => `<i style="display:block;width:12px;height:12px;border-radius:3px;background:${heatFill(v)}"></i>`).join('')}<span class="um" style="margin-left:4px">More</span></div>
    <div class="abs" id="sheet5" style="left:0;top:350px;width:352px;height:402px;border-radius:24px 24px 0 0;background:#1E1915;box-shadow:0 -1px 0 var(--line)">
      <div class="abs" style="left:156px;top:10px;width:40px;height:4px;border-radius:2px;background:var(--line)"></div>
      <div class="abs um" style="left:22px;top:32px">Saturday · September 12, 2026</div>
      <div class="abs uh" style="left:20px;top:52px;font-size:32px">That day</div>
      <div class="abs us" style="left:22px;top:96px">1 workout · 1 cardio session</div>
      ${[['Push A', '52 min · 18 sets'], ['Run', '5.2 km · 28:40']].map(([n, m], i) => `<div class="abs" style="left:22px;top:${130 + i * 58}px;width:308px;height:50px;border-bottom:1px solid var(--line)">
        <div class="abs ut" style="left:0;top:4px">${n}</div><div class="abs um" style="left:0;top:28px">${m}</div><div class="abs ut" style="right:0;top:12px;color:var(--muted)">→</div></div>`).join('')}</div>`,
  refs: (cam, r) => Object.assign(r, { ring: cam.querySelector('#ring5'), sheet: cam.querySelector('#sheet5') }),
  update: (t, r, B) => {
    // Tap the lit square for the 12th (row 1, column 5 in this grid) and the day's sheet comes up.
    const idx = 11, tap = bar(B + 1, .5);
    r.ring.style.transform = `translate(${24 + (idx % 7) * 44 - 2}px, ${86 + Math.floor(idx / 7) * 44 - 2}px) scale(${t >= tap ? 1 + .15 * Math.exp(-(t - tap) * 10) : 1})`;
    r.ring.style.opacity = t >= tap - .05 ? 1 : 0;
    const sk = E.expo(prog(t, tap + .1, .55));
    r.sheet.style.transform = `translateY(${(1 - sk) * 420}px)`;
  },
});

/* 06 · Cleaner settings */
feature(5, {
  pal: 'kettle', title: 'Cleaner settings', line: 'Eight pages trade walls of pills for titles, switches and steppers.',
  before: `
    <div class="sb">9:41</div>
    <div class="abs" style="left:20px;top:46px;font-size:20px;color:var(--muted)">←</div>
    <div class="abs um" style="left:20px;top:86px;color:var(--ink)">Session</div>
    <div class="abs ut" style="left:20px;top:116px;font-size:14px">Feedback strength</div>
    <div class="abs" style="left:20px;top:140px;width:312px">${['Off', 'Light', 'Medium', 'Strong'].map(p => `<span class="pl" style="${p === 'Medium' ? 'border-color:var(--accent);color:var(--accent)' : ''}">${p}</span>`).join('')}</div>
    <div class="abs ut" style="left:20px;top:190px;font-size:14px">Compound lifts</div>
    <div class="abs" style="left:20px;top:214px;width:312px">${['2:00', '2:30', '3:00', '3:30', '4:00', '5:00'].map(p => `<span class="pl" style="${p === '3:00' ? 'border-color:var(--accent);color:var(--accent)' : ''}">${p}</span>`).join('')}</div>
    ${[0, 1, 2].map(j => `<div class="abs" style="left:20px;top:${290 + j * 88}px;width:312px">
      <div style="width:${110 + j * 20}px;height:9px;border-radius:5px;background:rgba(242,239,234,.35);margin-bottom:14px"></div>
      ${Array.from({ length: 6 + j }, (_, i) => `<span class="pl" style="width:${24 + (i % 3) * 8}px;height:10px"></span>`).join('')}</div>`).join('')}`,
  after: `
    <div class="sb">9:41</div>
    <div class="abs" style="left:20px;top:46px;font-size:20px;color:var(--muted)">←</div>
    <div class="abs uh" style="left:18px;top:80px;font-size:36px">Session</div>
    <div class="abs us" style="left:20px;top:126px">Applies while you log a workout.</div>
    <div class="abs um" style="left:20px;top:166px">Feedback strength</div>
    <div class="abs" style="left:20px;top:186px;width:312px;height:40px;border-radius:12px;background:var(--tile);display:flex;padding:4px;box-sizing:border-box;gap:4px">${['Off', 'Light', 'Medium', 'Strong'].map(p => `<span class="ut" style="flex:1;display:flex;align-items:center;justify-content:center;border-radius:9px;font-size:13px;${p === 'Medium' ? 'background:var(--ink);color:#110F0C;font-weight:500' : 'color:var(--muted)'}">${p}</span>`).join('')}</div>
    <div class="abs" style="left:20px;top:250px;width:312px;height:52px;border-bottom:1px solid var(--line)"><span class="abs ut" style="left:0;top:14px">Compound lifts</span><span class="abs" style="right:0;top:8px">${stepr('2:00', 20, 'cmp')}</span></div>
    <div class="abs" style="left:20px;top:312px;width:312px;height:52px;border-bottom:1px solid var(--line)"><span class="abs ut" style="left:0;top:14px">Isolation lifts</span><span class="abs" style="right:0;top:8px">${stepr('1:30', 20)}</span></div>`,
  refs: (cam, r) => Object.assign(r, { cmp: cam.querySelector('#cmp') }),
  update: (t, r, B) => {
    const tap = bar(B + 1, 1);
    r.cmp.textContent = t >= tap ? '2:30' : '2:00';
    r.cmp.style.transform = `scale(${t >= tap ? 1 + .18 * Math.exp(-(t - tap) * 9) : 1})`;
  },
});

/* ── Also changed, one per beat ─────────────────────────────────────────────────────────────────── */
const MED = [
  ['Check-in waits in the bell', 'No pop-up at launch. A banner flies to the bell.'],
  ['A quieter Home card', 'The target lines are gone, and the reason is small text.'],
  ['Real rest days', 'Home says Rest today and names your next session.'],
  ['Smarter rest timer', 'Compounds start at 2:00. Heavy sets get more, light sets less.'],
  ['One-line exercise plan', '3 × 8-12 · last 135 × 10 · try 140 lb'],
  ['Better generated plans', 'No two moves in a row train the same muscle.'],
  ['Weights in your unit', 'Suggestions land on real steps: 62.5 kg, not 60.1.'],
  ['Tidier stats', 'Bigger headers. Single-set exercises skip the chart.'],
  ['Onboarding refresh', 'New animations and redrawn gym and gear icons.'],
  ['Export shows progress', 'Workouts written so far, with Cancel.'],
  ['Import fixes', 'Hevy, Strong and FitNotes: weights, cardio and dates.'],
  ['Stronger app locks', 'Turning a lock off asks for your fingerprint or PIN.'],
  ['Watch fixes', 'Heart rate on newer watches, and clear retry messages.'],
];
scene(15, 19, cam => {
  cam.innerHTML = `
    <div class="abs mono" style="left:160px;top:236px">Also changed</div>
    <div class="abs" style="left:0;top:0">
      ${MED.map(([a, b]) => `<div class="abs li" style="left:0;top:0;transform-origin:0 50%"><div class="serif" style="font-size:104px">${a}</div><div class="mono sub" style="margin-top:14px;font-size:20px;letter-spacing:.12em">${b}</div></div>`).join('')}
    </div>
    <div class="abs" id="mark" style="left:520px;top:0;width:12px;height:12px;border-radius:50%;background:var(--accent)"></div>`;
  return { items: [...cam.querySelectorAll('.li')], mark: cam.querySelector('#mark'),
           cast: [mkChar(cam, CH.nori, 180), mkChar(cam, CH.mochi, 160), mkChar(cam, CH.kettle, 170), mkChar(cam, CH.momo, 160)] };
}, (t, r) => {
  let pos = 0;
  for (let i = 1; i < MED.length; i++) pos += E.expo(prog(t, bar(15, i) - LEAD, .28));
  const base = 480;
  r.items.forEach((el, i) => {
    const d = i - pos, ad = Math.abs(d);
    const sc = ad < 1 ? lerp(1, .42, ad) : .42;
    const y = base + d * 132 + (d > 0 ? 70 : 0) * clamp(ad);
    el.style.transform = `translate(580px, ${y}px) scale(${sc})`;
    el.style.opacity = clamp(1 - Math.max(0, ad - .3) * .45) * (i === 0 ? E.out(prog(t, bar(15) - .05, .3)) : 1);
    el.querySelector('.sub').style.opacity = clamp(1 - ad * 2);
    el.firstElementChild.style.color = ad < .5 ? 'var(--ink)' : 'var(--muted)';
  });
  r.mark.style.transform = `translateY(${base + 52}px) scale(${1 + .5 * Math.exp(-beatPhase(t) * 8)})`;
  const xs = [220, 390, 1580, 1750];
  r.cast.forEach((c, i) => {
    const off = (i % 2) * BEAT;
    const arms = (Math.floor((t + off) / BEAT) % 2) ? [150, 30] : [30, 150];
    c.set(xs[i], 1000, { mood: i % 2 ? 'cheer' : 'happy', ...Act.bounce(t + off, 14), armR: arms[0], armL: arms[1], rot: 5 * Math.sin(2 * Math.PI * (t + off) / (2 * BEAT)),
                         blink: Act.blink(t, i), wig: wig(t, 4, 12), look: [i < 2 ? 3 : -3, -1] });
  });
});

/* ── Small details, ticked off on eighths ───────────────────────────────────────────────────────── */
const SMALL = [
  'Warm-up’s “Start lifting” stays dim until every drill is ticked',
  'The check-in reads “Weight (kg)” and saves in kg',
  'Records and e1RM show 7.5 kg, not 7',
  'This week’s arrows only show when last week had data',
  'The last exercise says “finish or add another”',
  'Recent shows custom cardio by its name',
  'Leaving a workout with only notes now asks first',
  'Back closes Cardio’s screens one at a time',
  'Switching kg/lb in onboarding converts your bodyweight',
  'Re-rolling mid-workout asks “Discard & continue”',
  'Body fat from Health Connect can now be granted',
  'The “That day” sheet scrolls on busy days',
  'The rest-over buzz respects Haptics Off',
  'A quick action that did nothing is gone',
  'Custom exercises keep the muscle you picked',
];
scene(19, 22, cam => {
  cam.innerHTML = `
    <div class="abs mono" style="left:160px;top:200px">And the small stuff</div>
    <div class="abs serif" style="left:156px;top:240px;font-size:80px">Small details</div>
    ${SMALL.map((s, i) => `<div class="abs sm" style="left:${i < 8 ? 160 : 990}px;top:${380 + (i % 8) * 62}px;width:780px;height:50px">
      <svg viewBox="0 0 28 28" style="position:absolute;left:0;top:4px;width:28px;height:28px"><circle cx="14" cy="14" r="12.5" fill="none" stroke="var(--line)" stroke-width="2"/>
        <path class="ck" d="M8 14.5 L12.5 19 L20.5 10" fill="none" stroke="#E23D3D" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" pathLength="1" stroke-dasharray="1 1" stroke-dashoffset="1"/></svg>
      <div class="abs sans" style="left:44px;top:4px;font-size:23px;color:var(--ink);white-space:nowrap">${s}</div></div>`).join('')}`;
  return { rows: [...cam.querySelectorAll('.sm')] };
}, (t, r) => {
  r.rows.forEach((row, i) => {
    const at = bar(19, 1 + i / 2);
    const k = E.expo(prog(t, at - LEAD, .4));
    row.style.opacity = k; row.style.transform = `translateX(${(1 - k) * 30}px)`;
    row.querySelector('.ck').style.strokeDashoffset = 1 - E.out(prog(t, at, .25));
  });
});

/* ── End ─────────────────────────────────────────────────────────────────────────────────────────── */
scene(22, 24, cam => {
  cam.innerHTML = `
    <div class="abs" style="left:0;top:0;width:1920px;height:1080px;background:radial-gradient(900px 560px at 50% 40%, rgba(226,61,61,.12), transparent 70%)"></div>
    <div class="abs" id="mk" style="left:600px;top:270px;width:190px;height:160px"></div>
    <div class="abs serif" id="word" style="left:820px;top:250px;font-size:190px;letter-spacing:-.04em;font-weight:500;font-variation-settings:'SOFT' 100">${words('Avex')}</div>
    <div class="abs serif red" id="ver" style="left:1300px;top:290px;font-size:84px">1.0</div>
    <div class="abs serif" id="thx" style="left:0;width:1920px;top:560px;font-size:64px;text-align:center">${lines(['Thanks for _testing._'])}</div>`;
  const mk = cam.querySelector('#mk'); mk.innerHTML = markSVG(); drawMark(mk, 1, 1);
  return { mk, word: cam.querySelector('#word'), ver: cam.querySelector('#ver'), thx: cam.querySelector('#thx'),
           cast: [mkChar(cam, CH.mochi, 220), mkChar(cam, CH.nori, 250), mkChar(cam, CH.kettle, 230), mkChar(cam, CH.momo, 220)] };
}, (t, r) => {
  const B = 22;
  const mk = E.expo(prog(t, bar(B) - LEAD, .6));
  r.mk.style.opacity = mk; r.mk.style.transform = `scale(${lerp(.8, 1, mk)})`;
  io(r.word, t, bar(B) - LEAD, 1e9, .05, .7);
  const vk = E.back(prog(t, bar(B, 1) - LEAD, .4));
  r.ver.style.transform = `scale(${vk})`; show(r.ver, vk > 0 ? 1 : 0);
  ioLines(r.thx, t, [bar(B, 2) - LEAD]);
  const xs = [620, 840, 1080, 1300];
  r.cast.forEach((c, i) => {
    const up = spring(t - bar(B) + .1 - i * .06, 1.3, 5);
    c.set(xs[i], lerp(1500, 1000, up), { mood: 'cheer', ...Act.cheer(t - bar(B) + i * .1, 26), fx: 'sparkles', time: t + i, wig: wig(t, 3, 12), ...(i === 1 ? Act.wave(t) : {}) }, t > bar(B) - .1 + i * .06 ? 1 : 0);
  });
});

/* ═══ Cues, on the grid ════════════════════════════════════════════════════════════════════════ */
cue(bar(1, 2), 'pop', .8);                                                          // 0.9 → 1.0
cue(bar(2), 'tick', .7);                                                            // the numbers
for (let k = 0; k < 6; k++) cue(bar(4 + 2 * k) - .3, 'whoosh', .9);                 // each seam lands on its bar
cue(bar(4, 1), 'tap'); cue(bar(4, 1), 'confirm', .7);                               // Log set
cue(bar(6) + 1.2, 'chime', .6);                                                     // the lesson's accent lands
cue(bar(8, .5), 'pop', .8);                                                         // the prompt slides in
cue(bar(10, 1), 'tap'); cue(bar(10, 2.5), 'tick', .8);                              // tap Pull, pick up Upper
cue(bar(12, .5), 'tap');                                                            // tap the 12th
cue(bar(14, 1), 'tap');                                                             // compound rest +30 s
for (let i = 1; i < MED.length; i++) cue(bar(15, i), 'tap', .5);                    // each row lands
SMALL.forEach((_, i) => cue(bar(19, 1 + i / 2), 'tick', .35));                      // each detail ticked
cue(bar(22), 'chime', .7);                                                          // the end card

/* ═══ Render ═══════════════════════════════════════════════════════════════════════════════════ */
const overlay = html(`<div class="abs" style="inset:0;pointer-events:none"><div class="vignette"></div><div class="grain"></div><div class="fade"></div></div>`);
stage.appendChild(overlay);
const fadeEl = overlay.querySelector('.fade');

function render(t) {
  t = clamp(t, 0, DUR - 1e-4);
  for (const sc of SCENES) {
    let vis = t >= sc.from && t < sc.to, x = 0;
    if (PUSH_AT.includes(sc.from) && t >= sc.from - PUSH_LEN && t < sc.from) { vis = true; x = W * (1 - E.inOut(prog(t, sc.from - PUSH_LEN, PUSH_LEN))); }
    if (PUSH_AT.includes(sc.to) && t >= sc.to - PUSH_LEN && t < sc.to) { x = -W * E.inOut(prog(t, sc.to - PUSH_LEN, PUSH_LEN)); }
    sc.root.style.display = vis ? 'block' : 'none';
    if (!vis) continue;
    sc.shake = [0, 0];
    sc.update(t, sc.refs, sc);
    const z = lerp(sc.zoom[0], sc.zoom[1], E.inOut(prog(t, sc.from, sc.to - sc.from)));
    sc.cam.style.transform = `translate(${x + sc.shake[0]}px, ${sc.shake[1]}px) scale(${z})`;
  }
  fadeEl.style.opacity = Math.max(1 - clamp(t / .25), clamp((t - (DUR - 1)) / 1));
}

window.__seek = t => { render(t); return true; };
window.__meta = { DUR, FPS, W, H, CUES, BAR, CUTS: SCENES.map(s => s.from) };

/* ═══ Viewer ═══════════════════════════════════════════════════════════════════════════════════ */
const params = new URLSearchParams(location.search);
if (params.has('render')) document.body.classList.add('render');
const frameEl = document.getElementById('frame');
function fit() { const s = document.body.classList.contains('render') ? 1 : frameEl.clientWidth / W; stage.style.transform = `scale(${s})`; }
addEventListener('resize', fit); fit();

const playBtn = document.getElementById('v-play'), scrub = document.getElementById('v-scrub'), clockEl = document.getElementById('v-clock');
scrub.max = DUR.toFixed(2);
const audio = new Audio('audio/changelog-mix.mp3');
audio.preload = 'auto';
let playing = false, t0 = 0, clock0 = 0, cur = 0;
const fmt = t => `${Math.floor(t / 60)}:${(t % 60).toFixed(1).padStart(4, '0')}`;
function setT(t) { cur = clamp(t, 0, DUR); render(cur); scrub.value = cur; clockEl.textContent = fmt(cur); }
function tick() {
  if (!playing) return;
  const t = audio.paused || audio.readyState < 2 ? clock0 + (performance.now() - t0) / 1000 : audio.currentTime;
  if (t >= DUR) { pause(); setT(DUR - .001); return; }
  setT(t);
  requestAnimationFrame(tick);
}
function play() {
  if (cur >= DUR - .05) cur = 0;
  playing = true; playBtn.textContent = 'Pause';
  clock0 = cur; t0 = performance.now();
  try { audio.currentTime = cur; audio.play().catch(() => {}); } catch (e) {}
  requestAnimationFrame(tick);
}
function pause() { playing = false; playBtn.textContent = 'Play'; try { audio.pause(); } catch (e) {} }
playBtn.addEventListener('click', () => playing ? pause() : play());
scrub.addEventListener('input', () => { if (playing) pause(); setT(+scrub.value); });
addEventListener('keydown', e => {
  if (e.target.tagName === 'INPUT' && e.code !== 'Space') return;
  if (e.code === 'Space') { e.preventDefault(); playing ? pause() : play(); }
  if (e.code === 'ArrowRight') { pause(); setT(cur + 1 / FPS); }
  if (e.code === 'ArrowLeft') { pause(); setT(cur - 1 / FPS); }
});
// At rest the page shows the first before/after, mid-sweep being the whole idea.
setT(params.has('t') ? +params.get('t') : bar(4) + .8);
</script>
