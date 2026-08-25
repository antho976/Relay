import React from 'react';
import {Composition} from 'remotion';
import {Release, TOTAL} from './Release';
import {ONBOARDING_0_8_9, ONBOARDING_0_9, OnboardingBeat} from './Onboarding';
import {Compare} from './Beats';
import {WatchBeat} from './BeatWatch';
import {NoticeBeat} from './BeatNotice';
import {FPS, H, SEC, W} from './theme';

const dev = {fps: FPS, width: W, height: H} as const;

export const Root: React.FC = () => (
  <>
    <Composition id="release" component={Release} durationInFrames={TOTAL} {...dev} />

    {/* working compositions — each beat renderable on its own while it is being cut */}
    <Composition
      id="dev-onboarding" component={OnboardingBeat} durationInFrames={SEC(7)} {...dev}
      defaultProps={{before: ONBOARDING_0_8_9, after: ONBOARDING_0_9}}
    />
    <Composition
      id="dev-seam" component={Compare} durationInFrames={SEC(6.5)} {...dev}
      defaultProps={{
        before: {src: 'cfr/before-session.mp4', start: 690},
        after: {src: 'cfr/after-session.mp4', start: 890},
        eyebrow: 'The session', title: 'The screen you\nactually train on',
        note: 'The title stops eating the fold, last session and suggested next arrive together, and one button closes out an exercise.',
      }}
    />
    <Composition id="dev-watch" component={WatchBeat} durationInFrames={SEC(7.5)} {...dev} />
    <Composition id="dev-notice" component={NoticeBeat} durationInFrames={SEC(4.5)} {...dev} />
  </>
);
