import React from 'react';
import {AbsoluteFill, OffthreadVideo, staticFile} from 'remotion';

/** Same clip, three trims. If these three frames are identical, trimBefore is not being applied. */
export const TrimTest: React.FC = () => (
  <AbsoluteFill style={{backgroundColor: '#000', flexDirection: 'row', alignItems: 'center', justifyContent: 'space-around'}}>
    {[0, 380, 900].map((t) => (
      <div key={t} style={{position: 'relative'}}>
        <div style={{position: 'absolute', top: -34, left: 0, color: '#E23D3D', fontFamily: 'monospace', fontSize: 24}}>
          trimBefore={t}
        </div>
        <OffthreadVideo
          src={staticFile('clips/before-session.mp4')}
          trimBefore={t}
          muted
          style={{width: 260, height: 578, objectFit: 'cover'}}
        />
      </div>
    ))}
  </AbsoluteFill>
);
