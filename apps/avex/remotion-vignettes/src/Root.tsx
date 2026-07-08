import React from 'react';
import {Composition} from 'remotion';
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
  </>
);
