import React from 'react';
import {OffthreadVideo, Img, staticFile} from 'remotion';
import {OUTLINE, SHOT_H, SHOT_W, SURFACE} from './theme';

/**
 * A capture in a phone body. `bare` drops the bezel for edge-to-edge shots — the brief asked for
 * a mix, so the two live behind one prop rather than two components.
 */
export const Phone: React.FC<{
  src?: string;
  still?: string;
  height: number;
  startFrom?: number;
  /** <1 slows the clip, >1 speeds it. Used to make two unequal scrolls finish together. */
  playbackRate?: number;
  bare?: boolean;
  muted?: boolean;
}> = ({src, still, height, startFrom = 0, playbackRate = 1, bare = false, muted = true}) => {
  const scale = height / SHOT_H;
  const width = SHOT_W * scale;
  const radius = bare ? 18 : 46;
  const bezel = bare ? 0 : Math.round(height * 0.011);

  return (
    <div
      style={{
        width: width + bezel * 2,
        height: height + bezel * 2,
        borderRadius: radius + bezel,
        background: bare ? 'transparent' : '#000',
        padding: bezel,
        boxSizing: 'border-box',
        boxShadow: bare
          ? '0 24px 60px rgba(0,0,0,0.55)'
          : `0 0 0 1.5px ${OUTLINE}, 0 30px 80px rgba(0,0,0,0.65)`,
        flex: '0 0 auto',
      }}
    >
      <div
        style={{
          width,
          height,
          borderRadius: radius,
          overflow: 'hidden',
          background: SURFACE,
          position: 'relative',
        }}
      >
        {src ? (
          <OffthreadVideo
            src={staticFile(src)}
            trimBefore={startFrom}   /* `startFrom` is deprecated in Remotion 4.0.4xx */
            playbackRate={playbackRate}
            muted={muted}
            style={{width: '100%', height: '100%', objectFit: 'cover'}}
          />
        ) : still ? (
          <Img src={staticFile(still)} style={{width: '100%', height: '100%', objectFit: 'cover'}} />
        ) : null}
      </div>
    </div>
  );
};
