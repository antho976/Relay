import { Glow, Gradient, Grid, pop, rise, theme, useSafePadding, useSpring, Words } from '@relay/video';
import { linearTiming, TransitionSeries } from '@remotion/transitions';
import { fade } from '@remotion/transitions/fade';
import { AbsoluteFill, useVideoConfig } from 'remotion';
import type { Props } from './Root';

// Two scenes with a fade between them. Every element a check should name has a data-id.

const Intro: React.FC<Props> = ({ title, subtitle, accent }) => {
  const padding = useSafePadding();
  const sub = useSpring(16);
  const lastWord = title.trim().split(/\s+/).pop() ?? '';
  return (
    <AbsoluteFill>
      <Gradient />
      <Glow color={accent} />
      <Grid />
      <AbsoluteFill style={{ ...padding, alignItems: 'center', justifyContent: 'center', gap: 36 }}>
        <Words id="title" text={title} highlight={[lastWord]} highlightColor={accent} size={theme.type.display} />
        <div data-id="subtitle" style={{
          fontFamily: theme.fonts.body, fontSize: theme.type.subtitle, color: theme.colors.muted, textAlign: 'center',
          textWrap: 'balance', ...rise(sub, 24),
        }}>
          {subtitle}
        </div>
      </AbsoluteFill>
    </AbsoluteFill>
  );
};

const Outro: React.FC<Props> = ({ cta, accent }) => {
  const padding = useSafePadding();
  const p = useSpring(6, 14);
  return (
    <AbsoluteFill>
      <Gradient angle={200} />
      <Glow color={accent} y={0.55} />
      <AbsoluteFill style={{ ...padding, alignItems: 'center', justifyContent: 'center' }}>
        <div data-id="cta" style={{
          fontFamily: theme.fonts.display, fontSize: theme.type.title, fontWeight: 800, color: theme.colors.accentText,
          background: accent, padding: '28px 64px', borderRadius: 999, ...pop(p),
        }}>
          {cta}
        </div>
      </AbsoluteFill>
    </AbsoluteFill>
  );
};

export const Video: React.FC<Props> = (props) => {
  const { durationInFrames, fps } = useVideoConfig();
  const fadeFrames = Math.round(fps * 0.5);
  const intro = Math.round(durationInFrames * 0.6);
  const outro = durationInFrames - intro + fadeFrames;
  return (
    <TransitionSeries>
      <TransitionSeries.Sequence durationInFrames={intro} name="Intro">
        <Intro {...props} />
      </TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={fade()} timing={linearTiming({ durationInFrames: fadeFrames })} />
      <TransitionSeries.Sequence durationInFrames={outro} name="Outro">
        <Outro {...props} />
      </TransitionSeries.Sequence>
    </TransitionSeries>
  );
};
