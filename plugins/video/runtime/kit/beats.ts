// A music track's beat grid in frames: where each beat and bar falls, so every cut, entrance and
// sound lands on the music. Take the numbers from video.audio.beats, which answers the call to
// write (`useBeatGrid({ bpm, firstBeat, beatsPerBar })`). The grid is also what video.scan and
// video.timeline check cues and cuts against.
import { Internals, useVideoConfig } from 'remotion';

export type BeatGridOptions = {
  /** Beats a minute: 119.956. */
  bpm: number;
  /** When the first downbeat falls, in seconds (bar 1, beat 1). Default 0. */
  firstBeat?: number;
  /** Default 4. */
  beatsPerBar?: number;
};

export type BeatGrid = {
  bpm: number;
  fps: number;
  beatsPerBar: number;
  framesPerBeat: number;
  framesPerBar: number;
  /** The frame beat `n` falls on, from 0 at the first downbeat; fractions are between beats
   * (`beat(2.5)` is the "and" after the third). */
  beat: (n: number) => number;
  /** The frame bar `n` starts on, from 1 at the first downbeat; `bar(9.5)` is half through bar 9. */
  bar: (n: number) => number;
  /** Which beat `frame` is at (fractional), and which bar (from 1). */
  beatAt: (frame: number) => number;
  barAt: (frame: number) => number;
  /** The nearest beat's frame. */
  nearestBeat: (frame: number) => number;
  /** Whether `frame` is within `tolerance` frames (default 1) of a beat. */
  onBeat: (frame: number, tolerance?: number) => boolean;
  /** 1 on each beat, decaying to 0 over `decay` frames (default 8): to pulse something on the
   * beat. `accent` makes downbeats 1 and other beats that much. Every frame here is the
   * composition's (`useTimelineFrame()`), not a Sequence's own. */
  pulse: (frame: number, decay?: number, accent?: number) => number;
};

declare global {
  interface Window {
    __relay_grid?: { bpm: number; firstBeat: number; beatsPerBar: number; fps: number };
  }
}

/** The grid at `fps`; `useBeatGrid` is the same for the composition's own rate. */
export const beatGrid = ({ bpm, firstBeat = 0, beatsPerBar = 4 }: BeatGridOptions, fps: number): BeatGrid => {
  const framesPerBeat = (60 / bpm) * fps;
  const start = firstBeat * fps;
  const beat = (n: number) => Math.round(start + n * framesPerBeat);
  const beatAt = (frame: number) => (frame - start) / framesPerBeat;
  if (typeof window !== 'undefined') {
    window.__relay_grid = { bpm, firstBeat, beatsPerBar, fps };
  }
  return {
    bpm,
    fps,
    beatsPerBar,
    framesPerBeat,
    framesPerBar: framesPerBeat * beatsPerBar,
    beat,
    bar: (n: number) => beat((n - 1) * beatsPerBar),
    beatAt,
    barAt: (frame: number) => Math.floor(beatAt(frame) / beatsPerBar) + 1,
    nearestBeat: (frame: number) => beat(Math.round(beatAt(frame))),
    onBeat: (frame: number, tolerance = 1) => Math.abs(frame - beat(Math.round(beatAt(frame)))) <= tolerance,
    pulse: (frame: number, decay = 8, accent = 1) => {
      const at = beatAt(frame);
      if (at < 0) return 0;
      const n = Math.floor(at);
      const since = frame - beat(n);
      const strength = n % beatsPerBar === 0 ? 1 : accent;
      return since < 0 ? 0 : strength * Math.max(0, 1 - since / Math.max(1, decay));
    },
  };
};

/** The music's grid in the composition's frames. */
export const useBeatGrid = (options: BeatGridOptions): BeatGrid => {
  const { fps } = useVideoConfig();
  return beatGrid(options, fps);
};

/** The composition's frame, wherever this is drawn: inside a `<Sequence>` `useCurrentFrame()`
 * starts again at 0, but the music does not. Grids count in this frame. */
export const useTimelineFrame = (): number => Internals.useTimelinePosition();

/** 1 on each beat of `grid`, fading over `decay` frames, read at the composition's frame (so it
 * works inside a Sequence): `scale(1 + 0.04 * useBeatPulse(grid))`. */
export const useBeatPulse = (grid: BeatGrid, decay = 8, accent = 1): number => grid.pulse(useTimelineFrame(), decay, accent);
