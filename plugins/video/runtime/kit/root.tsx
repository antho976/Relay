// relayRoot: what a video's src/index.ts registers (docs/VIDEO.md). It draws the video's Root as
// it is, and beside it reads each composition's props schema for video.check and the props
// panel; it loads the kit's fonts and starts the probe video.still measures frames with.
import React from 'react';
import { Internals } from 'remotion';
import { z } from 'zod';
import { loadKitFonts } from './fonts';
import { installProbe, markCommitted } from './probe';

/** The prefix of the console line the compositions' schemas go out on. */
export const SCHEMA_PREFIX = '__relay_schema__';

const SchemaReader: React.FC = () => {
  const manager = React.useContext(Internals.CompositionManager);
  React.useEffect(() => {
    if (process.env.RELAY_PROBE !== '1') return;
    const schemas = manager.compositions.map((c) => {
      let schema: unknown = null;
      if (c.schema) {
        try {
          schema = z.toJSONSchema(c.schema as z.ZodType, { unrepresentable: 'any' });
        } catch (e) {
          schema = { error: String(e) };
        }
      }
      return { id: c.id, schema };
    });
    console.log(SCHEMA_PREFIX + JSON.stringify(schemas));
  }, [manager.compositions]);
  return null;
};

/** Records the frame each commit draws, for the probe: it is in the same tree as the composition
 * being drawn, so its layout effect runs in the commit that shows a new frame. */
const FrameMark: React.FC = () => {
  const { frame } = Internals.useTimelineContext();
  React.useLayoutEffect(() => {
    markCommitted(frame);
  });
  return null;
};

/** `registerRoot(relayRoot(Root))`: the video's Root, as Relay can see it. */
export const relayRoot = (Root: React.FC): React.FC => {
  installProbe();
  loadKitFonts();
  const RelayRoot: React.FC = () => (
    <>
      <Root />
      <SchemaReader />
      <FrameMark />
    </>
  );
  return RelayRoot;
};
