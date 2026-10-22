import { zColor } from '@remotion/zod-types';
import { Composition } from 'remotion';
import { z } from 'zod';
import { Video } from './Video';

// The props: what the person can change in Relay's props panel without touching the code.
// Describe each one; the description is the field's label there.
export const schema = z.object({
  title: z.string().describe('Headline'),
  subtitle: z.string().describe('Line under the headline'),
  cta: z.string().describe('Call to action'),
  accent: zColor().describe('Accent colour'),
});
export type Props = z.infer<typeof schema>;

export const Root: React.FC = () => (
  <Composition
    id="main"
    component={Video}
    schema={schema}
    defaultProps={{ title: __TITLE__, subtitle: 'Made with code, rendered by Relay', cta: 'Try it today', accent: '#7DD3FC' }}
    width={__WIDTH__}
    height={__HEIGHT__}
    fps={__FPS__}
    durationInFrames={__FRAMES__}
  />
);
