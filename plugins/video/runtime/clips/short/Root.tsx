// Written by Relay's Clips space: every clip of this VOD is the `short` composition, drawn from its
// edit as props (its length comes from the edit). Edits here are overwritten.
import { Composition } from 'remotion';
import { DEFAULT, metadata, schema, Short } from './Short';

export const Root: React.FC = () => (
  <Composition id="short" component={Short} schema={schema} defaultProps={DEFAULT} calculateMetadata={metadata}
    width={1080} height={1920} fps={30} durationInFrames={150} />
);
