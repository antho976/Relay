// relayRoot: what a video's src/index.ts registers (docs/VIDEO.md). It draws the video's Root as
// it is, and beside it reads each composition's props schema for video.check and the props
// panel; it loads the kit's fonts and starts the probe video.still measures frames with.
import React from 'react';
import { Internals } from 'remotion';
import { z } from 'zod';
import { loadKitFonts } from './fonts';
import { installProbe, markAssets, markCommitted, markSequences, type Sequenced, type SoundAsset } from './probe';

/** The prefix of the console line the compositions' schemas go out on. */
export const SCHEMA_PREFIX = '__relay_schema__';

const SchemaReader: React.FC = () => {
  const manager = React.useContext(Internals.CompositionManager);
  // A layout effect: in the commit that registers the compositions, before Remotion reads them
  // and its listing returns; an effect could run after, and the schemas would be missed.
  React.useLayoutEffect(() => {
    // Before the Root registers them there are none: nothing to say yet.
    if (process.env.RELAY_PROBE !== '1' || manager.compositions.length === 0) return;
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

/** Every Sequence the drawn frame holds, as Remotion's sequence manager has them: what
 * video.timeline lists as scenes. The manager keeps them outside the Studio only while asked to
 * (`SequenceRegistrationContext`), which relayRoot does when Relay is measuring. */
const SequenceReader: React.FC = () => {
  const { sequences } = React.useContext(Internals.SequenceManager);
  React.useLayoutEffect(() => {
    const list: Sequenced[] = (sequences as unknown as Record<string, unknown>[]).map((s) => ({
      id: String(s.id),
      parent: s.parent == null ? null : String(s.parent),
      type: String(s.type ?? 'sequence'),
      name: typeof s.displayName === 'string' ? s.displayName : '',
      from: Number(s.from ?? 0),
      duration: Number.isFinite(Number(s.duration)) ? Number(s.duration) : null,
      hidden: s.showInTimeline === false,
    }));
    markSequences(list);
  }, [sequences]);
  return null;
};

/** The sound Remotion will mix on this frame (every <Audio>, every clip with sound), read without
 * taking it from the renderer: what video.timeline places sounds the kit did not mark by. */
const AssetReader: React.FC = () => {
  const { renderAssets } = React.useContext(Internals.RenderAssetManager);
  React.useLayoutEffect(() => {
    const list: SoundAsset[] = [];
    for (const a of renderAssets as unknown as Record<string, unknown>[]) {
      if (a.type !== 'audio' && a.type !== 'video') continue;
      list.push({
        id: String(a.id), type: String(a.type), src: String(a.src), frame: Number(a.frame), volume: Math.round(Number(a.volume) * 1000) / 1000,
        media: Math.round(Number(a.mediaFrame) * 1000) / 1000, rate: Number(a.playbackRate ?? 1),
      });
    }
    markAssets(list);
  }, [renderAssets]);
  return null;
};

/** `registerRoot(relayRoot(Root))`: the video's Root, as Relay can see it. */
export const relayRoot = (Root: React.FC): React.FC => {
  installProbe();
  loadKitFonts();
  const measuring = process.env.RELAY_PROBE === '1' || process.env.RELAY_PROBE === 'timeline';
  const RelayRoot: React.FC = () => (
    <>
      {measuring ? (
        <Internals.SequenceRegistrationContext.Provider value>
          <Root />
          <SequenceReader />
          {process.env.RELAY_PROBE === 'timeline' ? <AssetReader /> : null}
        </Internals.SequenceRegistrationContext.Provider>
      ) : (
        <Root />
      )}
      <SchemaReader />
      <FrameMark />
    </>
  );
  return RelayRoot;
};
