import React from 'react';
import {Composition} from 'remotion';
import {Custom} from './Custom';
import {Freestyle} from './Freestyle';
import {Generated} from './Generated';
import {FPS, LOOP_FRAMES, VIGNETTE_HEIGHT, VIGNETTE_WIDTH} from './theme';

export const Root: React.FC = () => (
  <>
    <Composition
      id="generated"
      component={Generated}
      durationInFrames={LOOP_FRAMES}
      fps={FPS}
      width={VIGNETTE_WIDTH}
      height={VIGNETTE_HEIGHT}
    />
    <Composition
      id="custom"
      component={Custom}
      durationInFrames={LOOP_FRAMES}
      fps={FPS}
      width={VIGNETTE_WIDTH}
      height={VIGNETTE_HEIGHT}
    />
    <Composition
      id="freestyle"
      component={Freestyle}
      durationInFrames={LOOP_FRAMES}
      fps={FPS}
      width={VIGNETTE_WIDTH}
      height={VIGNETTE_HEIGHT}
    />
  </>
);
