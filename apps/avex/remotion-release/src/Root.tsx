import React from 'react';
import {Composition} from 'remotion';
import {Release, TOTAL} from './Release';
import {NativeProof} from './Native';
import {WatchBeat} from './NativeBeats';
import {TrimTest} from './Debug';
import {FPS, H, W} from './theme';

export const Root: React.FC = () => (
  <>
    <Composition id="release" component={Release} durationInFrames={TOTAL} fps={FPS} width={W} height={H} />
    <Composition id="native-proof" component={NativeProof} durationInFrames={150} fps={FPS} width={W} height={H} />
    <Composition id="watch" component={WatchBeat} durationInFrames={300} fps={FPS} width={W} height={H} />
    <Composition id="trimtest" component={TrimTest} durationInFrames={60} fps={FPS} width={W} height={H} />
  </>
);
