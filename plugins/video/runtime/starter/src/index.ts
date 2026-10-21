// Written by Relay: keep this file as it is. relayRoot draws Root unchanged, and lets video.still
// see each frame (element boxes, text, fonts) and the props panel read each schema.
import { relayRoot } from '@relay/video';
import { registerRoot } from 'remotion';
import { Root } from './Root';

registerRoot(relayRoot(Root));
