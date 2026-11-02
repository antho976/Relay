# Motion vocabulary

Turn a loose description ("the bouncy thing when a popover opens") into the exact term, so the owner
knows what to ask a designer or another model for. Read for the sensation described, not the
keywords. Lead with the term and its line, add one or two close alternatives with how they differ,
and stop:

```
**Rubber-banding**: resistance and snap-back when you drag past a boundary (the iOS overscroll feel).
```

When nothing fits exactly, give the closest term and say it's an approximation, or describe the
effect in these words ("a stagger of scale-in entrances"). Don't invent terms. How to build any of
these is in `motion.md`.

## Entrances and exits
- **Fade**: appears or disappears by opacity.
- **Slide in**: enters from off-screen along one edge.
- **Scale in**: grows from slightly smaller to full size, usually with a fade.
- **Pop in**: scales in with a slight overshoot, as if it bounces into place.
- **Reveal**: content uncovered gradually by a clip-path or mask.
- **Enter / exit**: the animation played when an element is added or removed.

## Sequencing and timing
- **Keyframes**: defined points (0%, 50%, 100%) the browser fills between.
- **Tween / interpolation**: generating the in-between frames from a start and end value.
- **Stagger**: several items animating one after another with a small delay, a cascade.
- **Orchestration**: timing several animations so they read as one motion.
- **Fill mode**: whether an element keeps its first or last frame's styles outside the animation.
- **Stepped animation**: progress in discrete steps, like a countdown.

## Movement
- **Skew**: slanting an element along an axis.
- **3D tilt / flip**: rotating in 3D (`rotateX`/`rotateY`) for depth. **Perspective** sets how strong
  the depth looks.
- **Transform origin**: the anchor a scale or rotation grows or spins from.
- **Origin-aware animation**: an element grows out of its trigger, like a popover from the button
  that opened it, instead of from its own centre.

## Between states
- **Crossfade**: one element fades out as another fades in, in the same spot.
- **Continuity transition**: before and after visibly connected so the person stays oriented, like
  one rectangle growing and shrinking.
- **Morph**: one shape smoothly becoming another (the Dynamic Island).
- **Shared element transition**: an element travels and transforms into its new place, like a
  thumbnail expanding into a card.
- **Layout animation**: a size or position change animates instead of snapping.
- **Accordion / collapse**: a section expanding and collapsing its height.
- **Direction-aware transition**: forward slides one way and back slides the other.

## Scroll
- **Scroll reveal**: elements fade or slide in as they enter the viewport.
- **Scroll-driven animation**: progress tied directly to scroll position.
- **Parallax**: layers moving at different speeds while scrolling.
- **Page transition** / **view transition**: motion between pages; a view transition has the browser
  morph between two states, connecting shared elements.

## Feedback and interaction
- **Press / tap feedback**: a subtle scale-down on press, so it feels physical.
- **Hold to confirm**: a fill that progresses while the button is held.
- **Drag**, **drag to reorder** (the others shift to make room), **swipe to dismiss**.
- **Rubber-banding**: resistance and snap-back past a boundary.
- **Shake / wiggle**: a quick side-to-side jitter for an error or rejected input.
- **Ripple**: a circle expanding from the tap point.

## Easing and springs
- **Ease-out** starts fast and ends slow; **ease-in** starts slow (feels sluggish); **ease-in-out**
  is slow, fast, slow; **linear** is constant speed.
- **Cubic-bezier**: a custom curve. **Asymmetric easing**: accelerating and decelerating at
  different rates.
- **Spring**: motion driven by physics rather than a set duration. **Stiffness / tension**: how
  hard it pulls to the target. **Damping**: how quickly it settles (less damping, more bounce).
  **Mass**: how heavy it feels. **Bounce**: overshoot before settling.
- **Perceptual duration**: when a spring looks finished, though it keeps micro-settling.
- **Momentum** / **velocity**: speed carried from a drag or into the next animation.
- **Interruptible animation**: one that can be redirected mid-flight instead of finishing first.

## Looping and ambient
- **Marquee**: content scrolling in a continuous loop.
- **Loop**; **alternate (yoyo)**: forward then reverse each iteration.
- **Orbit**: circling another element. **Pulse**: a repeating scale or opacity change to draw
  attention. **Float**: a gentle continuous drift. **Idle animation**: motion while nothing happens.

## Polish and effects
- **Blur**: a blur filter softening an element or masking a crossfade.
- **Clip-path**: clipping to a shape for reveals and before/after sliders. **Mask**: the same with
  soft, fadeable edges.
- **Before/after slider**: a draggable divider wiping between two overlaid images.
- **Line drawing**: an SVG path drawing itself in.
- **Text morph**: text animating character by character when it changes.
- **Skeleton / shimmer**: a placeholder with a moving sheen while content loads.
- **Number ticker**: digits rolling to a value. **Tabular numbers**: fixed-width digits so changing
  numbers don't shift.
- **Typewriter**: text appearing one character at a time.

## Performance
- **Frame rate**: frames per second (60 baseline, 120 on newer screens). **Jank** / **dropped frame**:
  visible stutter from frames that missed their deadline.
- **Compositing**: the GPU moving or fading an element on its own layer without layout or paint.
- **will-change**: a hint that promotes an element to its own layer before it animates.
- **Layout thrashing**: animating width, height, top or left, forcing layout every frame.

## Principles
- **Purposeful animation**: motion that orients, gives feedback or shows relationships.
- **Anticipation**: a small wind-up the opposite way before a move.
- **Follow-through**: parts settling slightly after the main motion stops.
- **Squash and stretch**: deforming a moving element to show weight and speed.
- **Perceived performance**: the right motion makes an interface feel faster.
- **Frequency of use**: the more often seen, the shorter and subtler.
- **Spatial consistency**: an element keeps its identity and position across states.
- **Reduced motion**: honouring the person's setting by toning motion down.
