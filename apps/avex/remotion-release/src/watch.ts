/**
 * The watch segment is the one beat with no before — `wear/` and `shared/` are new modules in 0.9.
 *
 * Set HAVE_FOOTAGE true once clips land in public/clips/watch-*.mp4 (or stills in
 * public/stills/watch/). Until then the beat is omitted entirely rather than faked: a rendered
 * mock-up of a watch app is a legitimate thing to ship in a launch film, but it must be a decision
 * someone makes on purpose, not a silent substitution for a capture that failed.
 */
export const HAVE_FOOTAGE = false;

/** Watch captures are square-ish; a Galaxy Watch reports 450×450 at 1x. */
export const WATCH_SHOT = {w: 450, h: 450};
