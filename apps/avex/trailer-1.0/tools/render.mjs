// Drive headless Chromium over the DevTools protocol and read frames out of index.html.
//
//   node tools/render.mjs stills out/stills 1.2 5.5 14.0 …   one PNG per time, for checking
//   node tools/render.mjs cues out/cues.json                 the sound cues the film declares
//   node tools/render.mjs video out/picture.mp4 [fps]        every frame, piped straight to ffmpeg
//
// The page is a pure function of time (window.__seek), so a frame here is exactly the frame the
// browser shows at that instant. Needs Node ≥ 22 (built-in WebSocket) and /usr/bin/chromium.
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
// PAGE=path/to/index.html renders another film with the same pipeline (the changelog uses this).
const page = 'file://' + resolve(process.env.PAGE || resolve(here, '..', 'index.html')) + '?render=1&t=0';
const [mode, out, ...rest] = process.argv.slice(2);
if (!mode || !out) { console.error('usage: render.mjs stills|cues|video <out> [...]'); process.exit(2); }

const port = 9300 + Math.floor(Math.random() * 500);
const chrome = spawn(process.env.CHROMIUM || '/usr/bin/chromium', [
  '--headless=new', `--remote-debugging-port=${port}`, '--hide-scrollbars', '--mute-audio',
  '--force-device-scale-factor=1', '--window-size=1920,1080', '--disable-gpu-vsync', '--run-all-compositor-stages-before-draw',
  `--user-data-dir=/tmp/avex-trailer-chrome-${port}`, 'about:blank',
], { stdio: ['ignore', 'ignore', 'pipe'] });

async function target() {
  for (let i = 0; i < 100; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      const p = list.find(x => x.type === 'page');
      if (p) return p.webSocketDebuggerUrl;
    } catch {}
    await new Promise(r => setTimeout(r, 100));
  }
  throw new Error('chromium did not come up');
}

const ws = new WebSocket(await target());
await new Promise(r => ws.addEventListener('open', r, { once: true }));
let id = 0;
const waiting = new Map();
const events = [];
ws.addEventListener('message', m => {
  const msg = JSON.parse(m.data);
  if (msg.id && waiting.has(msg.id)) {
    const { ok, no } = waiting.get(msg.id); waiting.delete(msg.id);
    msg.error ? no(new Error(JSON.stringify(msg.error))) : ok(msg.result);
  } else if (msg.method) events.push(msg);
});
const send = (method, params = {}) => new Promise((ok, no) => { const i = ++id; waiting.set(i, { ok, no }); ws.send(JSON.stringify({ id: i, method, params })); });
const evaluate = async expr => {
  const r = await send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  return r.result.value;
};

await send('Page.enable');
await send('Runtime.enable');
await send('Emulation.setDeviceMetricsOverride', { width: 1920, height: 1080, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: page });
for (let i = 0; i < 200 && !(await evaluate('typeof window.__seek === "function"').catch(() => false)); i++) await new Promise(r => setTimeout(r, 50));
await evaluate('document.fonts.ready.then(() => document.fonts.size)');
// Touch every face once so no frame is the first to ask for a font.
await evaluate('Promise.all(["400 40px Fraunces","italic 400 40px Fraunces","500 40px Fraunces","500 20px \\"IBM Plex Mono\\"","400 20px \\"IBM Plex Sans\\""].map(f => document.fonts.load(f))).then(() => true)');
const meta = await evaluate('window.__meta');

async function frame(t, format = 'png', quality) {
  await evaluate(`window.__seek(${t})`);
  const r = await send('Page.captureScreenshot', { format, quality, captureBeyondViewport: false, clip: { x: 0, y: 0, width: 1920, height: 1080, scale: 1 } });
  return Buffer.from(r.data, 'base64');
}

try {
  if (mode === 'cues') {
    mkdirSync(dirname(out), { recursive: true });
    writeFileSync(out, JSON.stringify({ dur: meta.DUR, bar: meta.BAR, fps: meta.FPS, cuts: meta.CUTS || [], cues: meta.CUES }, null, 1));
    console.log(`${meta.CUES.length} cues → ${out}`);
  } else if (mode === 'stills') {
    mkdirSync(out, { recursive: true });
    for (const s of rest) {
      const t = +s;
      writeFileSync(resolve(out, `t${t.toFixed(3).padStart(7, '0')}.png`), await frame(t));
    }
    console.log(`${rest.length} stills → ${out}`);
  } else if (mode === 'video') {
    const fps = +(rest[0] || meta.FPS);
    const n = Math.round(meta.DUR * fps);
    mkdirSync(dirname(out), { recursive: true });
    const ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(fps), '-c:v', 'mjpeg', '-i', '-',
      '-c:v', 'libx264', '-preset', 'slow', '-crf', '14', '-pix_fmt', 'yuv420p', '-tune', 'animation', '-movflags', '+faststart', out], { stdio: ['pipe', 'inherit', 'inherit'] });
    const started = Date.now();
    for (let f = 0; f < n; f++) {
      const buf = await frame(f / fps, 'jpeg', 96);
      if (!ff.stdin.write(buf)) await new Promise(r => ff.stdin.once('drain', r));
      if (f % 120 === 0) process.stdout.write(`\rframe ${f}/${n}  ${((Date.now() - started) / 1000).toFixed(0)}s`);
    }
    ff.stdin.end();
    await new Promise(r => ff.on('close', r));
    console.log(`\n${n} frames at ${fps} fps → ${out}`);
  }
} finally {
  ws.close();
  chrome.kill();
}
