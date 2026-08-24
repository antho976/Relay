import React from 'react';
import {AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig} from 'remotion';
import {Phone} from './Phone';
import {Body, Eyebrow, Plate, Title, VersionTag, useEdgeFade, useRise} from './Type';
import {ACCENT, MONO, MUTED, ON_BG, SERIF} from './theme';

/** Opening / closing card. */
export const TitleCard: React.FC<{eyebrow: string; title: string; sub?: string}> = ({
  eyebrow, title, sub,
}) => {
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 26}}>
          <Eyebrow delay={0}>{eyebrow}</Eyebrow>
          <Title delay={6} size={124}>{title}</Title>
          {sub ? (
            <div style={{fontFamily: MONO, fontSize: 24, letterSpacing: 3, color: MUTED, ...useRise(14)}}>
              {sub}
            </div>
          ) : null}
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

/**
 * Before | after, side by side. The two clips are trimmed independently: identical gestures do not
 * guarantee identical timing, so each side carries its own startFrom rather than assuming parity.
 */
export const Compare: React.FC<{
  beforeSrc: string; afterSrc: string;
  eyebrow: string; title: string; note?: string;
  beforeStart?: number; afterStart?: number;
  /** Frames of real motion in each clip. Given these, each side is retimed so both finish together
   *  — otherwise a long page (Profile: 774 frames) runs on while a short one (341) sits done. */
  beforeSpan?: number; afterSpan?: number;
  height?: number;
}> = ({
  beforeSrc, afterSrc, eyebrow, title, note,
  beforeStart = 0, afterStart = 0, beforeSpan, afterSpan, height = 760,
}) => {
  const {durationInFrames} = useVideoConfig();
  const HOLD = 18; // a beat of stillness at each end so nothing starts or stops mid-scroll
  const window_ = Math.max(1, durationInFrames - HOLD * 2);
  const rate = (span?: number) =>
    span && span > 0 ? Math.max(0.25, Math.min(3, span / window_)) : 1;
  const o = useEdgeFade(durationInFrames, 12);
  const head = useRise(0);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', width: '100%', height: '100%', alignItems: 'center'}}>
          <div style={{flex: '0 0 620px', paddingLeft: 96, display: 'flex', flexDirection: 'column', gap: 20}}>
            <Eyebrow delay={0}>{eyebrow}</Eyebrow>
            <Title delay={5} size={68}>{title}</Title>
            {note ? <Body delay={12}>{note}</Body> : null}
          </div>
          <div style={{flex: 1, display: 'flex', justifyContent: 'center', alignItems: 'center', gap: 74, ...head}}>
            <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 22}}>
              <Phone src={beforeSrc} height={height} startFrom={beforeStart} playbackRate={rate(beforeSpan)} />
              <VersionTag label="0.8.9" delay={8} />
            </div>
            <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 22}}>
              <Phone src={afterSrc} height={height} startFrom={afterStart} playbackRate={rate(afterSpan)} />
              <VersionTag label="0.9" accent delay={12} />
            </div>
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

/** One capture, large — for the things that have no before. */
export const Solo: React.FC<{
  src?: string; still?: string;
  eyebrow: string; title: string; note?: string;
  start?: number; bare?: boolean; height?: number;
}> = ({src, still, eyebrow, title, note, start = 0, bare = false, height = 880}) => {
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 12);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', width: '100%', height: '100%', alignItems: 'center'}}>
          <div style={{flex: '0 0 760px', paddingLeft: 120, display: 'flex', flexDirection: 'column', gap: 22}}>
            <Eyebrow delay={0}>{eyebrow}</Eyebrow>
            <Title delay={5} size={82}>{title}</Title>
            {note ? <Body delay={12}>{note}</Body> : null}
          </div>
          <div style={{flex: 1, display: 'flex', justifyContent: 'center', ...useRise(4)}}>
            <Phone src={src} still={still} height={height} startFrom={start} bare={bare} />
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

/** A run of stills, cross-faded — used for the onboarding flows, which are pages not motion. */
export const StillRun: React.FC<{
  files: string[]; eyebrow: string; title: string; note?: string; height?: number;
}> = ({files, eyebrow, title, note, height = 860}) => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 12);
  const per = durationInFrames / files.length;
  const idx = Math.min(files.length - 1, Math.floor(frame / per));
  const local = frame - idx * per;
  const fade = interpolate(local, [0, 8], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', width: '100%', height: '100%', alignItems: 'center'}}>
          <div style={{flex: '0 0 700px', paddingLeft: 120, display: 'flex', flexDirection: 'column', gap: 22}}>
            <Eyebrow delay={0}>{eyebrow}</Eyebrow>
            <Title delay={5} size={76}>{title}</Title>
            {note ? <Body delay={12}>{note}</Body> : null}
            <div style={{fontFamily: MONO, fontSize: 20, letterSpacing: 3, color: MUTED, marginTop: 8}}>
              {String(idx + 1).padStart(2, '0')} / {String(files.length).padStart(2, '0')}
            </div>
          </div>
          <div style={{flex: 1, display: 'flex', justifyContent: 'center', position: 'relative'}}>
            <div style={{opacity: fade}}>
              <Phone still={files[idx]} height={height} />
            </div>
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

/**
 * Two still-runs side by side, each paging at its own rate so both finish together. Used for the
 * onboarding flows: the point is that one is fifteen pages and the other is nine, which you only
 * feel if they run against the same clock.
 */
export const CompareStills: React.FC<{
  before: string[]; after: string[];
  eyebrow: string; title: string; note?: string; height?: number;
}> = ({before, after, eyebrow, title, note, height = 720}) => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 12);
  const pick = (files: string[]) => {
    const per = durationInFrames / files.length;
    const i = Math.min(files.length - 1, Math.floor(frame / per));
    const local = frame - i * per;
    const fade = interpolate(local, [0, 7], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
    return {file: files[i], fade, i};
  };
  const b = pick(before);
  const a = pick(after);

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', width: '100%', height: '100%', alignItems: 'center'}}>
          <div style={{flex: '0 0 600px', paddingLeft: 96, display: 'flex', flexDirection: 'column', gap: 20}}>
            <Eyebrow delay={0}>{eyebrow}</Eyebrow>
            <Title delay={5} size={68}>{title}</Title>
            {note ? <Body delay={12}>{note}</Body> : null}
          </div>
          <div style={{flex: 1, display: 'flex', justifyContent: 'center', alignItems: 'center', gap: 74}}>
            {[
              {p: b, label: `0.8.9 · ${String(before.length).padStart(2, '0')} pages`, accent: false},
              {p: a, label: `0.9 · ${String(after.length).padStart(2, '0')} pages`, accent: true},
            ].map((side, k) => (
              <div key={k} style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 22}}>
                <div style={{opacity: side.p.fade}}>
                  <Phone still={side.p.file} height={height} />
                </div>
                <VersionTag label={side.label} accent={side.accent} delay={8 + k * 4} />
              </div>
            ))}
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

/**
 * The small things, named. Some changes are real but too small to shoot — a chip's proportions, a
 * renamed section, a filter — and a list card is more honest than zooming on a detail nobody can
 * read at 1080p.
 */
export const ListCard: React.FC<{eyebrow: string; title: string; items: string[]}> = ({
  eyebrow, title, items,
}) => {
  const {durationInFrames} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);
  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', flexDirection: 'column', gap: 34, width: 1320}}>
          <Eyebrow delay={0}>{eyebrow}</Eyebrow>
          <Title delay={5} size={72}>{title}</Title>
          <div
            style={{
              display: 'grid', gridTemplateColumns: '1fr 1fr', columnGap: 76, rowGap: 20, marginTop: 10,
            }}
          >
            {items.map((it, i) => {
              const r = useRise(14 + i * 3);
              return (
                <div
                  key={i}
                  style={{
                    display: 'flex', gap: 16, alignItems: 'baseline',
                    fontFamily: SERIF, fontSize: 27, color: ON_BG, lineHeight: 1.4, ...r,
                  }}
                >
                  <span style={{fontFamily: MONO, fontSize: 17, color: ACCENT}}>
                    {String(i + 1).padStart(2, '0')}
                  </span>
                  <span>{it}</span>
                </div>
              );
            })}
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};

/**
 * One screen, held, with a spotlight that walks from one changed element to the next. Earlier this
 * drew every ring at once and the labels fought each other over the screen they were annotating —
 * one target at a time, with everything else dimmed, is what makes a small change legible.
 */
export const Spotlight: React.FC<{
  still?: string; src?: string; start?: number;
  eyebrow: string; title: string;
  rings: {x: number; y: number; label: string}[];
  height?: number;
}> = ({still, src, start = 0, eyebrow, title, rings, height = 900}) => {
  const frame = useCurrentFrame();
  const {durationInFrames, fps} = useVideoConfig();
  const o = useEdgeFade(durationInFrames, 14);

  const bezel = Math.round(height * 0.011);
  const innerW = (height / 2400) * 1080;

  const lead = 20;
  const per = Math.max(24, Math.floor((durationInFrames - lead - 14) / Math.max(1, rings.length)));
  const idx = Math.max(0, Math.min(rings.length - 1, Math.floor((frame - lead) / per)));
  const local = frame - lead - idx * per;
  const on = spring({frame: local, fps, config: {damping: 18, stiffness: 150}});
  const active = frame >= lead;

  // Ease between targets so the spotlight travels rather than jumping.
  const prev = rings[Math.max(0, idx - 1)];
  const cur = rings[idx];
  const t = interpolate(local, [0, 14], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const cx = bezel + ((prev.x + (cur.x - prev.x) * t) / 100) * innerW;
  const cy = bezel + ((prev.y + (cur.y - prev.y) * t) / 100) * height;
  const R = 78;

  return (
    <AbsoluteFill style={{opacity: o}}>
      <Plate>
        <div style={{display: 'flex', width: '100%', height: '100%', alignItems: 'center'}}>
          <div style={{flex: '0 0 640px', paddingLeft: 96, display: 'flex', flexDirection: 'column', gap: 22}}>
            <Eyebrow delay={0}>{eyebrow}</Eyebrow>
            <Title delay={5} size={62}>{title}</Title>
            <div style={{height: 132, marginTop: 14, position: 'relative'}}>
              {rings.map((r, i) => (
                <div
                  key={i}
                  style={{
                    position: 'absolute', top: 0, left: 0, display: 'flex', gap: 16, alignItems: 'baseline',
                    opacity: i === idx && active ? on : 0,
                    transform: `translateY(${(1 - (i === idx ? on : 0)) * 14}px)`,
                  }}
                >
                  <span style={{fontFamily: MONO, fontSize: 19, color: ACCENT}}>
                    {String(i + 1).padStart(2, '0')}
                  </span>
                  <span style={{fontFamily: SERIF, fontSize: 34, color: ON_BG, lineHeight: 1.3, maxWidth: 520}}>
                    {r.label}
                  </span>
                </div>
              ))}
            </div>
          </div>
          <div style={{flex: 1, display: 'flex', justifyContent: 'center'}}>
            <div style={{position: 'relative'}}>
              <Phone still={still} src={src} startFrom={start} height={height} />
              {/* dim everything but the target */}
              <div
                style={{
                  position: 'absolute', inset: 0, borderRadius: 46, pointerEvents: 'none',
                  background: `radial-gradient(circle ${R * 1.55}px at ${cx}px ${cy}px, rgba(17,15,12,0) 0%, rgba(17,15,12,0) 46%, rgba(17,15,12,0.52) 84%, rgba(17,15,12,0.56) 100%)`,
                  opacity: active ? 1 : 0,
                }}
              />
              <div
                style={{
                  position: 'absolute', left: cx - R, top: cy - R, width: R * 2, height: R * 2,
                  border: `3px solid ${ACCENT}`, borderRadius: '50%',
                  boxShadow: `0 0 18px ${ACCENT}33`,
                  opacity: active ? on : 0,
                  transform: `scale(${0.85 + 0.15 * on})`,
                }}
              />
            </div>
          </div>
        </div>
      </Plate>
    </AbsoluteFill>
  );
};
