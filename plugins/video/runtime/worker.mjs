// Relay's video worker (docs/VIDEO.md, D167). The engine keeps one per instance, started in the
// videos folder (whose node_modules this resolves against), and speaks JSON lines with it: a
// request {"id": n, "cmd": "...", ...} on stdin is answered {"id": n, "ok": true, "result": {...}}
// or {"id": n, "ok": false, "error": {"code", "message"}} on stdout, in whatever order they finish.
// A render's progress goes out as {"event": "progress", "job": "...", ...}. Logs go to stderr.
//
// `node worker.mjs ensure-browser` fetches Remotion's headless browser once instead, and exits.
//
// Written by Relay from plugins/video/runtime: edits here are overwritten.
import { bundle } from '@remotion/bundler';
import {
  ensureBrowser,
  getCompositions,
  makeCancelSignal,
  openBrowser,
  renderFrames,
  renderMedia,
  renderStill,
  selectComposition,
} from '@remotion/renderer';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const PROBE_PREFIX = '__relay_probe__';
const SCHEMA_PREFIX = '__relay_schema__';
const MOST_ERRORS = 50;
const MOST_FILES = 5000;
const LOG_LEVEL = 'error';

// stdout carries the protocol and nothing else: Remotion prints the page's console there, so
// every other write to it goes to stderr with the logs.
const protocol = process.stdout.write.bind(process.stdout);
process.stdout.write = (chunk, ...rest) => process.stderr.write(chunk, ...rest);
const out = (message) => protocol(JSON.stringify(message) + '\n');
const log = (...parts) => process.stderr.write(`[video-worker] ${parts.join(' ')}\n`);

class Refusal extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

// ---------------------------------------------------------------- the browser

let browser = null;

const getBrowser = async () => {
  browser ??= openBrowser('chrome', { logLevel: LOG_LEVEL }).catch((e) => {
    browser = null;
    throw new Refusal('video.browser', `Remotion's browser did not start: ${e.message}`);
  });
  return browser;
};

/** Run `f` with the browser; once more with a new one when the old one has died. */
const withBrowser = async (f) => {
  try {
    return await f(await getBrowser());
  } catch (e) {
    if (!/Target closed|Browser closed|Session closed|has been closed|Protocol error/i.test(String(e?.message))) throw e;
    log('browser gone, starting another:', e.message);
    browser = null;
    return f(await getBrowser());
  }
};

// ---------------------------------------------------------------- videos

/** What the worker keeps per video folder: its last bundle, and its TypeScript service. */
const videos = new Map();
const video = (dir) => {
  let v = videos.get(dir);
  if (!v) {
    v = { fingerprint: null, serveUrl: null, outDir: null, ls: null, files: [] };
    videos.set(dir, v);
  }
  return v;
};

const relative = (dir, file) => path.relative(dir, file).split(path.sep).join('/');
const inside = (dir, file) => {
  const r = path.relative(dir, file);
  return !!r && !r.startsWith('..') && !path.isAbsolute(r);
};

/** The files a bundle reads (src/ and public/), by size and change time: the same while nothing
 * was edited, so a still after a still does not bundle again. */
const fingerprint = (dir) => {
  const hash = createHash('sha1');
  let n = 0;
  const walk = (at) => {
    let entries;
    try {
      entries = fs.readdirSync(at, { withFileTypes: true });
    } catch {
      return;
    }
    entries.sort((a, b) => (a.name < b.name ? -1 : 1));
    for (const e of entries) {
      if (e.name.startsWith('.') || e.name === 'node_modules' || ++n > MOST_FILES) continue;
      const p = path.join(at, e.name);
      if (e.isDirectory()) walk(p);
      else {
        try {
          const s = fs.statSync(p);
          hash.update(`${p}\0${s.size}\0${s.mtimeMs}\n`);
        } catch {
          /* gone meanwhile */
        }
      }
    }
  };
  walk(path.join(dir, 'src'));
  walk(path.join(dir, 'public'));
  return hash.digest('hex').slice(0, 16);
};

// One bundle at a time: webpack is greedy with memory, and two of one folder would race.
let bundling = Promise.resolve();
/** Bundles in use by a render, which a newer bundle must not remove from under it. */
const inUse = new Map();

/** The video's bundle (a served folder), built again only when its files changed. */
const bundleVideo = (dir, bundleRoot) => {
  const run = bundling.then(async () => {
    const v = video(dir);
    const print = fingerprint(dir);
    if (v.serveUrl && v.fingerprint === print && fs.existsSync(v.serveUrl)) return v;
    const outDir = path.join(bundleRoot, print);
    const entryPoint = path.join(dir, 'src', 'index.ts');
    if (!fs.existsSync(entryPoint)) throw new Refusal('video.entry', 'src/index.ts is missing: it registers the video (registerRoot(relayRoot(Root)))');
    const started = performance.now();
    const serveUrl = await bundle({
      entryPoint,
      outDir,
      publicDir: path.join(dir, 'public'),
      enableCaching: true,
      onProgress: () => {},
    });
    log(`bundled ${relative(ROOT, dir)} in ${Math.round(performance.now() - started)} ms`);
    Object.assign(v, { fingerprint: print, serveUrl, outDir });
    // Older bundles of this video go, but not one a render still reads.
    for (const old of safeReaddir(bundleRoot)) {
      const p = path.join(bundleRoot, old);
      if (p !== outDir && !inUse.get(p)) fs.rmSync(p, { recursive: true, force: true });
    }
    return v;
  });
  bundling = run.catch(() => {});
  return run;
};

const safeReaddir = (dir) => {
  try {
    return fs.readdirSync(dir);
  } catch {
    return [];
  }
};

// ---------------------------------------------------------------- errors, where they are in the code

/** A bundle's failure as errors in files: esbuild's `file:line:col: ERROR: …` and webpack's
 * `ERROR in ./src/X.tsx line:col` / `Module not found` lines. */
const bundleErrors = (e, dir) => {
  const text = String(e?.message ?? e);
  const found = [];
  for (const m of text.matchAll(/^(\/[^\n:]+):(\d+):(\d+): ERROR: (.+)$/gm)) {
    found.push({ kind: 'syntax', file: relative(dir, m[1]), line: +m[2], col: +m[3] + 1, message: m[4].trim() });
  }
  if (found.length === 0) {
    for (const m of text.matchAll(/Module not found: Error: Can't resolve '([^']+)' in '([^']+)'/g)) {
      found.push({ kind: 'bundle', file: inside(dir, m[2]) ? relative(dir, m[2]) : undefined, message: `Cannot find ${m[1]} (imported from ${inside(dir, m[2]) ? relative(dir, m[2]) : m[2]})` });
    }
  }
  if (found.length === 0) {
    const first = text.split('\n').find((l) => l.trim()) ?? 'the bundle failed';
    found.push({ kind: /Transform failed|Unexpected|Expected/.test(text) ? 'syntax' : 'bundle', message: first.trim().slice(0, 500) });
  }
  return found;
};

const consumers = new Map();
/** Where a position in a bundle file came from in the video's code. */
const original = async (outDir, url, line, column) => {
  if (!outDir || !url || !line) return null;
  const name = path.basename(String(url).split('?')[0]);
  const mapFile = path.join(outDir, `${name}.map`);
  let consumer = consumers.get(mapFile);
  if (!consumer) {
    if (!fs.existsSync(mapFile)) return null;
    const { SourceMapConsumer } = require('source-map');
    consumer = await new SourceMapConsumer(JSON.parse(fs.readFileSync(mapFile, 'utf8')));
    consumers.set(mapFile, consumer);
  }
  const pos = consumer.originalPositionFor({ line, column: Math.max(0, (column ?? 1) - 1) });
  if (!pos?.source) return null;
  let source = pos.source.replace(/^webpack:\/\/[^/]*\//, '').replace(/^webpack:\/\//, '');
  source = path.isAbsolute(source) ? source : path.resolve(ROOT, source);
  return { file: source, line: pos.line, col: (pos.column ?? 0) + 1 };
};

/** A failure while drawing (or listing compositions) as an error in the video's code, at the
 * first stack frame that is the video's own. */
const runtimeError = async (e, dir, outDir, frame) => {
  const message = String(e?.message ?? e).split('\n')[0].slice(0, 500);
  const at = { kind: 'runtime', message, ...(frame === undefined ? {} : { frame }) };
  for (const f of e?.symbolicatedStackFrames ?? []) {
    const file = f?.originalFileName && path.resolve(ROOT, f.originalFileName);
    if (file && inside(dir, file)) return { ...at, file: relative(dir, file), line: f.originalLineNumber, col: f.originalColumnNumber };
  }
  for (const f of e?.stackFrame ?? []) {
    const pos = await original(outDir, f.fileName, f.lineNumber, f.columnNumber);
    if (pos && inside(dir, pos.file) && !pos.file.includes('node_modules')) {
      return { ...at, file: relative(dir, pos.file), line: pos.line, col: pos.col };
    }
  }
  return at;
};

// ---------------------------------------------------------------- TypeScript

let ts = null;
let registry = null;

const mtime = (file) => {
  try {
    const s = fs.statSync(file);
    return `${s.mtimeMs}:${s.size}`;
  } catch {
    return '0';
  }
};

/** TypeScript's errors in the video's own files, from a service kept per video so a second check
 * reads only what changed. */
const typecheck = (dir) => {
  ts ??= require('typescript');
  registry ??= ts.createDocumentRegistry();
  const v = video(dir);
  const config = path.join(dir, 'tsconfig.json');
  if (!fs.existsSync(config)) return [{ kind: 'type', file: 'tsconfig.json', message: 'tsconfig.json is missing: it extends ../../tsconfig.base.json' }];
  const parsed = ts.getParsedCommandLineOfConfigFile(config, {}, { ...ts.sys, onUnRecoverableConfigFileDiagnostic: () => {} });
  if (!parsed) return [{ kind: 'type', file: 'tsconfig.json', message: 'tsconfig.json cannot be read' }];
  v.files = parsed.fileNames;
  v.options = parsed.options;
  v.ls ??= ts.createLanguageService({
    getScriptFileNames: () => v.files,
    getScriptVersion: mtime,
    getScriptSnapshot: (f) => (fs.existsSync(f) ? ts.ScriptSnapshot.fromString(fs.readFileSync(f, 'utf8')) : undefined),
    getCurrentDirectory: () => dir,
    getCompilationSettings: () => v.options,
    getDefaultLibFileName: (o) => ts.getDefaultLibFilePath(o),
    fileExists: ts.sys.fileExists,
    readFile: ts.sys.readFile,
    readDirectory: ts.sys.readDirectory,
    directoryExists: ts.sys.directoryExists,
    getDirectories: ts.sys.getDirectories,
    realpath: ts.sys.realpath,
  }, registry);
  const errors = [];
  for (const file of v.files) {
    if (!inside(dir, file)) continue;
    const found = [...v.ls.getSyntacticDiagnostics(file), ...v.ls.getSemanticDiagnostics(file)];
    for (const d of found) {
      if (d.category !== ts.DiagnosticCategory.Error) continue;
      const at = d.file && d.start !== undefined ? d.file.getLineAndCharacterOfPosition(d.start) : null;
      errors.push({
        kind: 'type',
        file: relative(dir, d.file?.fileName ?? file),
        ...(at ? { line: at.line + 1, col: at.character + 1 } : {}),
        message: `TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, ' ')}`.slice(0, 500),
      });
      if (errors.length >= MOST_ERRORS) return errors;
    }
  }
  return errors;
};

// ---------------------------------------------------------------- commands

const summary = (c) => ({
  id: c.id,
  width: c.width,
  height: c.height,
  fps: c.fps,
  duration_frames: c.durationInFrames,
  default_props: c.defaultProps ?? {},
});

const probeOf = (logs, prefix) => {
  const line = [...logs].reverse().find((l) => l.text?.startsWith(prefix));
  if (!line) return null;
  try {
    return JSON.parse(line.text.slice(prefix.length));
  } catch {
    return null;
  }
};

const consoleErrors = (logs) => logs
  .filter((l) => l.type === 'error' && !l.text?.startsWith(PROBE_PREFIX))
  .map((l) => String(l.text).slice(0, 300))
  .slice(0, 10);

const PROBE_ENV = { RELAY_PROBE: '1' };

const commands = {
  async hello() {
    return {
      node: process.version,
      remotion: require('remotion/package.json').version,
      typescript: require('typescript/package.json').version,
    };
  },

  /** Build, typecheck, and list the compositions with their schemas. */
  async check({ dir, bundle_dir }) {
    const errors = [];
    let v = null;
    try {
      v = await bundleVideo(dir, bundle_dir);
    } catch (e) {
      if (e instanceof Refusal) throw e;
      errors.push(...bundleErrors(e, dir));
    }
    try {
      errors.push(...typecheck(dir));
    } catch (e) {
      log('typecheck failed:', e.stack ?? e);
    }
    let compositions = [];
    if (v) {
      const logs = [];
      try {
        const list = () => withBrowser((puppeteerInstance) => getCompositions(v.serveUrl, {
          puppeteerInstance, logLevel: LOG_LEVEL, envVariables: PROBE_ENV, inputProps: {}, onBrowserLog: (l) => logs.push(l),
        }));
        // The fullest list the page reported: it says them again as compositions register.
        const fullest = () => logs
          .filter((l) => l.text?.startsWith(SCHEMA_PREFIX))
          .map((l) => { try { return JSON.parse(l.text.slice(SCHEMA_PREFIX.length)); } catch { return null; } })
          .filter(Array.isArray)
          .reduce((best, list) => (best === null || list.length >= best.length ? list : best), null);
        let found = await list();
        // A page's console can go unheard (the browser busy with other pages): ask once more
        // before saying relayRoot is missing.
        if (!fullest()) {
          found = await list();
        }
        const schemas = fullest();
        compositions = found.map((c) => ({ ...summary(c), props_schema: schemas?.find((s) => s.id === c.id)?.schema ?? null }));
        if (!schemas) {
          errors.push({ kind: 'probe', file: 'src/index.ts', message: 'src/index.ts does not register relayRoot(Root): put back `registerRoot(relayRoot(Root))`, or stills cannot see the frame and the props panel has no schema' });
        }
      } catch (e) {
        errors.push(await runtimeError(e, dir, v.outDir));
      }
    }
    return { errors: errors.slice(0, MOST_ERRORS), compositions };
  },

  /** A composition's size, rate and length with these props (cheap: the bundle is kept). */
  async meta({ dir, bundle_dir, composition, props }) {
    let v;
    try {
      v = await bundleVideo(dir, bundle_dir);
    } catch (e) {
      if (e instanceof Refusal) throw e;
      return { errors: bundleErrors(e, dir) };
    }
    try {
      const c = await withBrowser((puppeteerInstance) => selectComposition({
        serveUrl: v.serveUrl, id: composition, inputProps: props ?? {}, puppeteerInstance, logLevel: LOG_LEVEL,
      }));
      return { composition: summary(c), errors: [] };
    } catch (e) {
      if (/Could not find composition/i.test(String(e?.message))) throw new Refusal('video.composition', String(e.message).split('\n')[0]);
      return { errors: [await runtimeError(e, dir, v.outDir)] };
    }
  },

  /** Draw `frames` to `outputs`, measuring each. */
  async still({ dir, bundle_dir, composition, frames, outputs, scale, props }) {
    let v;
    try {
      v = await bundleVideo(dir, bundle_dir);
    } catch (e) {
      if (e instanceof Refusal) throw e;
      return { stills: [], errors: bundleErrors(e, dir) };
    }
    const inputProps = props ?? {};
    let c;
    try {
      c = await withBrowser((puppeteerInstance) => selectComposition({ serveUrl: v.serveUrl, id: composition, inputProps, puppeteerInstance, logLevel: LOG_LEVEL }));
    } catch (e) {
      if (/Could not find composition/i.test(String(e?.message))) throw new Refusal('video.composition', String(e.message).split('\n')[0]);
      return { stills: [], errors: [await runtimeError(e, dir, v.outDir)] };
    }
    const stills = [];
    const errors = [];
    for (let i = 0; i < frames.length; i++) {
      const frame = Math.min(frames[i], c.durationInFrames - 1);
      const logs = [];
      try {
        await withBrowser((puppeteerInstance) => renderStill({
          serveUrl: v.serveUrl, composition: c, frame, output: outputs[i], scale, inputProps, puppeteerInstance,
          logLevel: LOG_LEVEL, envVariables: PROBE_ENV, imageFormat: 'png', overwrite: true, onBrowserLog: (l) => logs.push(l),
        }));
        // Drawn but not heard measuring (a console line can go unheard): draw it once more.
        if (!probeOf(logs, PROBE_PREFIX)) {
          await withBrowser((puppeteerInstance) => renderStill({
            serveUrl: v.serveUrl, composition: c, frame, output: outputs[i], scale, inputProps, puppeteerInstance,
            logLevel: LOG_LEVEL, envVariables: PROBE_ENV, imageFormat: 'png', overwrite: true, onBrowserLog: (l) => logs.push(l),
          }));
        }
        stills.push({ frame, path: outputs[i], probe: probeOf(logs, PROBE_PREFIX), console: consoleErrors(logs) });
      } catch (e) {
        errors.push(await runtimeError(e, dir, v.outDir, frame));
        stills.push({ frame, path: null, probe: null, console: consoleErrors(logs) });
      }
    }
    return { composition: summary(c), stills, errors };
  },

  /** Measure every `every` frames of `range` with the probe and no pictures: video.scan. */
  async scan({ dir, bundle_dir, composition, props, every, range }) {
    let v;
    try {
      v = await bundleVideo(dir, bundle_dir);
    } catch (e) {
      if (e instanceof Refusal) throw e;
      return { probes: [], errors: bundleErrors(e, dir) };
    }
    const inputProps = props ?? {};
    const logs = [];
    try {
      const c = await withBrowser((puppeteerInstance) => selectComposition({ serveUrl: v.serveUrl, id: composition, inputProps, puppeteerInstance, logLevel: LOG_LEVEL }));
      await withBrowser((puppeteerInstance) => renderFrames({
        serveUrl: v.serveUrl, composition: c, inputProps, imageFormat: 'none', everyNthFrame: every ?? 3, frameRange: range ?? null,
        envVariables: PROBE_ENV, onBrowserLog: (l) => logs.push(l), onStart: () => {}, onFrameUpdate: () => {}, outputDir: null,
        puppeteerInstance, logLevel: LOG_LEVEL,
      }));
    } catch (e) {
      if (/Could not find composition/i.test(String(e?.message))) throw new Refusal('video.composition', String(e.message).split('\n')[0]);
      return { probes: [], errors: [await runtimeError(e, dir, v.outDir)] };
    }
    const probes = [];
    for (const l of logs) {
      if (!l.text?.startsWith(PROBE_PREFIX)) continue;
      try {
        probes.push(JSON.parse(l.text.slice(PROBE_PREFIX.length)));
      } catch {
        /* a line cut short */
      }
    }
    return { probes, console: [...new Set(consoleErrors(logs))], errors: [] };
  },

  /** Render the whole composition (or `range`) to `out`, reporting progress as it goes. */
  async render({ job, dir, bundle_dir, composition, format, out: output, scale, crf, props, range }) {
    let v;
    try {
      v = await bundleVideo(dir, bundle_dir);
    } catch (e) {
      if (e instanceof Refusal) throw e;
      const [first] = bundleErrors(e, dir);
      throw new Refusal('video.build', `${first.file ? `${first.file}:${first.line ?? ''} ` : ''}${first.message}`);
    }
    const outDir = v.outDir;
    inUse.set(outDir, (inUse.get(outDir) ?? 0) + 1);
    const { cancelSignal, cancel } = makeCancelSignal();
    running.set(job, cancel);
    try {
      const inputProps = props ?? {};
      const c = await withBrowser((puppeteerInstance) => selectComposition({ serveUrl: v.serveUrl, id: composition, inputProps, puppeteerInstance, logLevel: LOG_LEVEL }));
      const codec = { mp4: 'h264', webm: 'vp8', gif: 'gif', prores: 'prores', mp3: 'mp3', wav: 'wav' }[format];
      if (!codec) throw new Refusal('video.format', `no format ${format}`);
      let last = 0;
      const options = {
        serveUrl: v.serveUrl, composition: c, codec, outputLocation: output, scale, inputProps, logLevel: LOG_LEVEL,
        cancelSignal, overwrite: true, frameRange: range ?? null,
        onProgress: (p) => {
          const now = performance.now();
          if (now - last < 400 && p.progress < 1) return;
          last = now;
          out({ event: 'progress', job, progress: p.progress, rendered: p.renderedFrames, encoded: p.encodedFrames, stage: p.stitchStage });
        },
      };
      if (crf !== undefined && crf !== null && (codec === 'h264' || codec === 'vp8')) options.crf = crf;
      if (codec === 'prores') Object.assign(options, { proResProfile: '4444', imageFormat: 'png', pixelFormat: 'yuva444p10le' });
      if (codec === 'gif') Object.assign(options, { everyNthFrame: c.fps >= 30 ? 2 : 1, numberOfGifLoops: 0 });
      const started = performance.now();
      await withBrowser((puppeteerInstance) => renderMedia({ ...options, puppeteerInstance }));
      const frames = range ? range[1] - range[0] + 1 : c.durationInFrames;
      return { path: output, bytes: fs.statSync(output).size, frames, ms: Math.round(performance.now() - started) };
    } catch (e) {
      if (e instanceof Refusal) throw e;
      if (/cancel/i.test(String(e?.message))) throw new Refusal('video.render.cancelled', 'the render was stopped');
      const at = await runtimeError(e, dir, outDir);
      throw new Refusal('video.render.failed', `${at.file ? `${at.file}:${at.line ?? ''} ` : ''}${at.frame !== undefined ? `(frame ${at.frame}) ` : ''}${at.message}`);
    } finally {
      running.delete(job);
      const n = (inUse.get(outDir) ?? 1) - 1;
      if (n <= 0) inUse.delete(outDir);
      else inUse.set(outDir, n);
    }
  },

  async cancel({ job }) {
    const cancel = running.get(job);
    if (cancel) cancel();
    return { cancelled: !!cancel };
  },

  /** Forget a video (deleted): its bundle and service. */
  async forget({ dir }) {
    videos.delete(dir);
    return {};
  },
};

const running = new Map();

// Looking at videos (check, meta, still, scan) goes one request at a time: pages opened side by
// side have been seen to lose each other's console lines, which is how the probe reports. A
// render runs beside them; it needs no console.
const LOOKS = new Set(['check', 'meta', 'still', 'scan']);
let looking = Promise.resolve();
const inTurn = (f) => {
  const run = looking.then(f, f);
  looking = run.catch(() => {});
  return run;
};

// ---------------------------------------------------------------- the loop

const serve = () => {
  const lines = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  lines.on('line', async (line) => {
    if (!line.trim()) return;
    let request;
    try {
      request = JSON.parse(line);
    } catch {
      log('unreadable request:', line.slice(0, 200));
      return;
    }
    const { id, cmd, ...args } = request;
    const handler = commands[cmd];
    if (!handler) {
      out({ id, ok: false, error: { code: 'video.worker', message: `no command ${cmd}` } });
      return;
    }
    try {
      out({ id, ok: true, result: await (LOOKS.has(cmd) ? inTurn(() => handler(args)) : handler(args)) });
    } catch (e) {
      if (!(e instanceof Refusal)) log(cmd, 'failed:', e?.stack ?? e);
      out({ id, ok: false, error: { code: e?.code ?? 'video.worker', message: String(e?.message ?? e).slice(0, 2000) } });
    }
  });
  // The engine closes stdin to stop the worker; anything still running goes with it.
  lines.on('close', async () => {
    for (const cancel of running.values()) cancel();
    try {
      await (await browser)?.close({ silent: true });
    } catch {
      /* already gone */
    }
    process.exit(0);
  });
};

if (process.argv[2] === 'ensure-browser') {
  try {
    await ensureBrowser({ logLevel: LOG_LEVEL });
    out({ ok: true });
    process.exit(0);
  } catch (e) {
    out({ ok: false, error: String(e?.message ?? e) });
    process.exit(1);
  }
} else {
  serve();
}
