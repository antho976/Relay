import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {HomeScreen} from './ui/HomeScreen';
import {C} from './ui/tokens';
import {Eyebrow, Plate, Title} from './Type';

/** Proof beat: one Home screen walking 0.8.9 → 0.9, with the era label tracking the same clock. */
export const NativeProof: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps, durationInFrames} = useVideoConfig();

  const t = spring({frame: frame - 40, fps, config: {damping: 200, stiffness: 34}});
  const fill = interpolate(frame, [10, 60], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const lift = interpolate(t, [0, 1], [0, -10]);

  return (
    <AbsoluteFill style={{backgroundColor: C.bg}}>
      <Plate>
        <div style={{display: 'flex', width: '100%', height: '100%', alignItems: 'center'}}>
          <div style={{flex: '0 0 700px', paddingLeft: 110, display: 'flex', flexDirection: 'column', gap: 22}}>
            <Eyebrow delay={0}>Home</Eyebrow>
            <Title delay={4} size={72}>One screen,{'\n'}two eras</Title>
            <div style={{display: 'flex', gap: 18, marginTop: 18, alignItems: 'center'}}>
              <Era label="0.8.9" on={1 - t} />
              <div style={{width: 220, height: 3, borderRadius: 999, background: '#38302A', position: 'relative'}}>
                <div style={{position: 'absolute', inset: 0, width: `${t * 100}%`, background: C.accent, borderRadius: 999}} />
              </div>
              <Era label="0.9" on={t} accent />
            </div>
          </div>
          <div style={{flex: 1, display: 'flex', justifyContent: 'center'}}>
            <div style={{transform: `translateY(${lift}px)`, filter: 'drop-shadow(0 40px 90px rgba(0,0,0,.6))'}}>
              <HomeScreen width={430} t={t} goalFill={fill} />
            </div>
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

const Era: React.FC<{label: string; on: number; accent?: boolean}> = ({label, on, accent}) => (
  <div
    style={{
      fontFamily: '"JetBrains Mono", ui-monospace, monospace', fontSize: 20, letterSpacing: 3,
      color: accent ? C.accent : C.muted, opacity: 0.35 + on * 0.65,
      border: `1.5px solid ${accent ? C.accent : '#38302A'}`, borderRadius: 999, padding: '8px 20px',
    }}
  >
    {label}
  </div>
);
