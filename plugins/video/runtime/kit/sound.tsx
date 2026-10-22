// Sound: one table of levels, a cue placed at a frame, voice lines, and music that ducks under
// the voice. Each marks itself for Relay's checks (an empty, hidden element), so video.scan and
// video.timeline know which sound is music, voice or an effect, exactly when it starts and how
// loud it plays: cues off the beat grid, voice lines over each other, music that does not drop
// under a voice and long silences are found without anyone listening.
import React from 'react';
import { Audio, Sequence, interpolate, staticFile, useCurrentFrame } from 'remotion';

/** How loud each kind of sound plays, as a volume (1 is the file as it is). Effects files are
 * best normalised to the same peak first (-3 dBFS), so this table is the only place the mix is
 * shaped: transients sit higher than tones, beds well under the voice. */
export const LEVELS = {
  music: 0.45,
  voice: 1,
  sfx: 0.6,
  ui: 0.4,
  whoosh: 0.4,
  impact: 0.7,
  ambience: 0.25,
} as const;
export type Level = keyof typeof LEVELS;

/** A span of frames, start and end: `[30, 95]`. */
export type Span = [number, number];

/** `src` as Remotion takes it: a `staticFile()` URL as it is, `assets/…` through `staticFile`. */
const source = (src: string) => (src.startsWith('/') || src.includes('://') || src.startsWith('blob:') ? src : staticFile(src));

const fileName = (src: string) => src.split('/').pop()?.replace(/\.[a-z0-9]+$/i, '') ?? src;

/** The volume multiplier at `frame` for music under `spans` (voice lines): down `db` dB while a
 * span plays, ramping down over `attack` frames before it and back up over `release` after. */
export const duck = (frame: number, spans: Span[], { db = -9, attack = 6, release = 10 }: { db?: number; attack?: number; release?: number } = {}): number => {
  const clamp = (v: number) => Math.max(0, Math.min(1, v));
  let depth = 0;
  for (const [a, b] of spans) {
    const into = attack > 0 ? clamp((frame - (a - attack)) / attack) : frame >= a ? 1 : 0;
    const out = release > 0 ? clamp((b + release - frame) / release) : frame <= b ? 1 : 0;
    depth = Math.max(depth, Math.min(into, out));
  }
  return 1 - (1 - 10 ** (db / 20)) * depth;
};

/** Spans of voice lines placed at `at` for `frames` each: what `Music`'s `duckUnder` takes. */
export const voiceSpans = (lines: { at: number; frames: number }[]): Span[] => lines.map((l) => [l.at, l.at + l.frames]);

type Placed = { frames?: number; trimBefore?: number; playbackRate?: number };

/** What the checks read of a sound on this frame: what it is, how long it has played, how loud it
 * is now, and how much of its file it plays (the engine reads the file's own length). Hidden and
 * empty: nothing is drawn. */
const Mark: React.FC<{ role: string; src: string; name: string; volume: number } & Placed> = ({ role, src, name, volume, frames, trimBefore, playbackRate }) => {
  const elapsed = useCurrentFrame();
  return (
    <span hidden data-relay-sound={role} data-src={src} data-name={name} data-elapsed={elapsed} data-volume={Math.round(volume * 1000) / 1000}
      data-frames={frames} data-trim={trimBefore} data-rate={playbackRate} />
  );
};

const fades = (f: number, frames: number | undefined, fadeIn: number, fadeOut: number) => {
  let v = 1;
  if (fadeIn > 0) v *= interpolate(f, [0, fadeIn], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
  if (fadeOut > 0 && frames) v *= interpolate(f, [frames - fadeOut, frames], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
  return v;
};

type Common = {
  /** `staticFile("assets/x.wav")`, or the `assets/x.wav` it names. */
  src: string;
  /** The frame it starts on, in the sequence it is placed in. */
  at?: number;
  /** Frames it plays for at most (default: the file's length). */
  frames?: number;
  /** Frames of the file to skip at its start. */
  trimBefore?: number;
  fadeIn?: number;
  fadeOut?: number;
  /** What the checks and the timeline call it; default the file's name. */
  name?: string;
  playbackRate?: number;
};

/** A sound effect at frame `at`, as loud as its `level` in [LEVELS] says (or a volume of its own),
 * times `gain`. */
export const Cue: React.FC<Common & { level?: Level | number; gain?: number }> = ({
  src, at = 0, frames, trimBefore, fadeIn = 0, fadeOut = 0, name, playbackRate, level = 'sfx', gain = 1,
}) => {
  const base = (typeof level === 'number' ? level : LEVELS[level]) * gain;
  const url = source(src);
  return (
    <Sequence from={Math.round(at)} durationInFrames={frames} layout="none" name={`${typeof level === 'string' ? level : 'sfx'} ${name ?? fileName(url)}`}>
      <Audio src={url} trimBefore={trimBefore} playbackRate={playbackRate} volume={(f) => base * fades(f, frames, fadeIn, fadeOut)} />
      <CueMark role={typeof level === 'string' ? level : 'sfx'} src={url} name={name ?? fileName(url)} volume={(f) => base * fades(f, frames, fadeIn, fadeOut)}
        frames={frames} trimBefore={trimBefore} playbackRate={playbackRate} />
    </Sequence>
  );
};

const CueMark: React.FC<{ role: string; src: string; name: string; volume: (f: number) => number } & Placed> = ({ volume, ...rest }) => {
  const f = useCurrentFrame();
  return <Mark {...rest} volume={volume(f)} />;
};

/** A voice line at frame `at`, at full level unless `volume` says otherwise. */
export const Voice: React.FC<Common & { volume?: number }> = ({ src, at = 0, frames, trimBefore, fadeIn = 0, fadeOut = 0, name, playbackRate, volume = LEVELS.voice }) => {
  const url = source(src);
  return (
    <Sequence from={Math.round(at)} durationInFrames={frames} layout="none" name={`voice ${name ?? fileName(url)}`}>
      <Audio src={url} trimBefore={trimBefore} playbackRate={playbackRate} volume={(f) => volume * fades(f, frames, fadeIn, fadeOut)} />
      <CueMark role="voice" src={url} name={name ?? fileName(url)} volume={(f) => volume * fades(f, frames, fadeIn, fadeOut)}
        frames={frames} trimBefore={trimBefore} playbackRate={playbackRate} />
    </Sequence>
  );
};

/** A music bed from frame `at` (default 0) at `LEVELS.music` (or `volume`), ducking `duckDb` dB
 * (default -9) under the `duckUnder` spans — the voice lines, in the same frames as `at`. Fade it
 * out over the last half second rather than cutting it mid-note. */
export const Music: React.FC<Common & { volume?: number; duckUnder?: Span[]; duckDb?: number; attack?: number; release?: number; loop?: boolean }> = ({
  src, at = 0, frames, trimBefore, fadeIn = 0, fadeOut = 0, name, playbackRate, volume = LEVELS.music, duckUnder = [], duckDb = -9, attack = 6,
  release = 10, loop,
}) => {
  const url = source(src);
  const level = (f: number) => volume * fades(f, frames, fadeIn, fadeOut) * duck(f + at, duckUnder, { db: duckDb, attack, release });
  return (
    <Sequence from={Math.round(at)} durationInFrames={frames} layout="none" name={`music ${name ?? fileName(url)}`}>
      <Audio src={url} trimBefore={trimBefore} playbackRate={playbackRate} loop={loop} volume={level} />
      <CueMark role="music" src={url} name={name ?? fileName(url)} volume={level} frames={frames} trimBefore={trimBefore} playbackRate={playbackRate} />
    </Sequence>
  );
};
