# Motion

The one source for how things move: whether they move at all, the values, gestures, performance and
reduced motion. The owner's own motion rules (`../SKILL.md`: intros may show off, working screens
never idle) sit on top. Code for common web components is in `motion-recipes.md`, React Native in
`react-native.md`, and the review, audit, find and remove workflows in `motion-passes.md`.

Distilled from Emil Kowalski's animation and design-engineering writing and Apple's WWDC talks
(*Designing Fluid Interfaces*, *The Details of UI Typography*, *Designing Audio-Haptic Experiences*).

## 1. Should it move at all

Decide this before anything else. Writing no animation is often the right result.

| How often it is seen | Decision |
| --- | --- |
| 100+ times a day: keyboard shortcuts, command palette, tab switches, core navigation | **None.** Raycast has no open/close animation, and that is correct |
| Tens of times a day: hover, list navigation, frequent toggles, row selection | Near-imperceptible (opacity or colour, ≤ 150 ms) or none |
| Occasional: modals, sheets, toasts, settings | Standard motion |
| Rare: first run, empty state, success, celebration, a startup beat | The delight budget lives here |

- **Keyboard-initiated actions never animate.** That is a rule, not a judgement call.
- **Tabs never slide.** Tabs are peers, so sliding suggests a depth that isn't there.
- **Data people read or act on does not move for style.** A chart doesn't redraw from zero on each
  visit, and a number doesn't count up on load. A value may tween from old to new when the person
  changed it.
- **Name the purpose in one word:** feedback, spatial consistency (where it came from or went),
  state indication, preventing a jarring change, explanation (onboarding and marketing only), or
  delight (rare tier only). If you can't name one, leave it out. "It looks nice" isn't a purpose on
  anything seen often.
- **Personality sets how far it goes.** A tool used all day is crisp and quick. A playful consumer
  surface may bounce. All of the owner's apps are tools.

## 2. The values

Every curve, duration and spring comes from here. Never hand-roll a curve that only looks
familiar. When a project already has motion tokens (`--ease-*`, `Motion.kt`, a duration scale),
use and extend them instead of adding a parallel set.

**Curves.** The built-in CSS and platform easings are too weak for deliberate motion.

```css
--ease-out:    cubic-bezier(0.23, 1, 0.32, 1);   /* entering, exiting, anything responding to the person */
--ease-in-out: cubic-bezier(0.77, 0, 0.175, 1);  /* something already on screen moving from A to B */
--ease-drawer: cubic-bezier(0.32, 0.72, 0, 1);   /* sheets and drawers (the iOS sheet curve) */
```

| Situation | Easing |
| --- | --- |
| Entering or exiting | `--ease-out`, with the exit faster than the entrance |
| Moving or morphing on screen | `--ease-in-out` |
| Hover, colour change | `ease` |
| Constant motion, progress fill, hold-to-confirm | `linear` |
| Anything else | `--ease-out` |

**Never `ease-in` on UI, exits included.** It starts slow, so it delays the moment the person is
watching. `ease-out` at 200 ms feels faster than `ease-in` at 200 ms. If no curve here fits, take one
from easing.dev or easings.co.

**Durations.** UI motion stays under 300 ms. Only large surfaces go beyond that.

| Element | Duration |
| --- | --- |
| Press feedback, hover, toggle, colour | 100–160 ms |
| Tooltip, small popover | 125–200 ms |
| Dropdown, menu, select | 150–250 ms |
| Modal, dialog | 200–300 ms |
| Sheet, drawer, toast, large surface | 300–400 ms (or a spring, below) |
| Screen-to-screen navigation | The platform's own (iOS push is 350 ms); never rebuilt |
| Intro, showcase, explanation | Longer, once, then out of the way |

A 180 ms dropdown feels more responsive than a 400 ms one, and a faster spinner makes the same load
feel shorter.

**Sizes.** Enter from `scale(0.95–0.97)` with opacity 0, never `scale(0)`: nothing in the real world
appears from nothing. Rise 6–12 px when entering. Press is `scale(0.97)` (0.95–0.98).

**Stagger.** Items 30–60 ms apart with about 300 ms in total, so roughly six items. Beyond that, fade
the group in as one. A stagger is decorative: it never blocks input, and it isn't used on a list
people load many times a day.

**Springs** for anything a finger or pointer drives, and for anything that may be interrupted.
Think in Apple's two parameters: the damping ratio (1 means no overshoot, lower is bouncier) and
the response (seconds to reach the target, which is not a duration).

| Interaction | Damping | Response | Motion (`type: "spring"`) | Reanimated |
| --- | --- | --- | --- | --- |
| Default settle, move, reposition | 1.0 | 0.3–0.4 s | `bounce: 0, duration: 0.4` | `{ duration: 400, dampingRatio: 1 }` |
| Release after a flick or drag | 0.8 | 0.4 s | `bounce: 0.2, duration: 0.4` | `{ duration: 400, dampingRatio: 0.8, velocity }` |
| Sheet, drawer | 0.8 | 0.3 s | `bounce: 0.2, duration: 0.3` | `{ duration: 300, dampingRatio: 0.8, velocity }` |
| Rotation | 0.8 | 0.4 s | `bounce: 0.2, duration: 0.4` | `{ duration: 400, dampingRatio: 0.8 }` |

Bounce only when the gesture carried momentum: overshoot on a card you flicked feels right, on a
menu that faded in it feels wrong. Keep bounce between 0.1 and 0.3. Use `overshootClamping` where a
hard edge must not be passed. On Compose, use the project's `Motion.kt` springs.

**Asymmetric timing.** Slow where the person is deciding, quick where the system answers: a
hold-to-delete fills over 2 s `linear` and snaps back in 200 ms `--ease-out` on release.

**One set per product.** One curve family and one duration scale, kept as tokens. Five hand-typed
cubic-beziers that almost match is a finding.

## 3. Pick the tool: the cheapest that works

| Need | Web | Native |
| --- | --- | --- |
| Hover, press, colour, a toggled state | CSS transition | Compose `animate*AsState`; Reanimated CSS transition; GTK CSS transition |
| Entry on mount without JS state | `@starting-style` (fallback: a `data-mounted` flag set after mount) | `AnimatedVisibility`; Reanimated `entering` |
| Predetermined motion that must stay smooth while the page is busy | CSS animation (it runs off the main thread) | n/a |
| Programmatic control, no library | WAAPI, `element.animate()` | n/a |
| Springs, layout and exit animations, gesture-driven values | Motion (motion.dev) | Compose `Animatable` and springs; Reanimated shared values |

Use CSS for predetermined motion and JS for dynamic or interruptible motion. Don't install a
library for a fade. If the request is really a component (toast, drawer, command menu, dropdown),
use a component library rather than hand-building one (`libraries.md`). Hand-built ones end up as a
`<div>` dropdown with no focus handling.

## 4. Properties and performance

Each frame has 16.7 ms at 60 Hz and **8.3 ms at 120 Hz**, which most phones and many laptops now
run. Design against the smaller budget, on a mid-range phone, not the developer's laptop.

| Tier | Properties | Cost |
| --- | --- | --- |
| Composite only | `transform`, `opacity` (current Chrome and Firefox also `filter`, `clip-path`, `background-color`) | Cheapest, can run off the main thread |
| Paint + composite | `box-shadow`, `border-radius`, `color` | A redraw every frame |
| Layout + paint + composite | `width`, `height`, `padding`, `margin`, `top`, `left`, `border-width`, `gap` | Re-measures the element and everything beside it |

- **Animate `transform` and `opacity`.** `clip-path` is the sanctioned extra (reveals, see the
  recipes). `height` is tolerated only for an accordion, short, to a measured height rather than
  `auto`. An absolutely positioned element with no children (a tab pill, a progress fill) may
  animate `width`, because nothing else re-lays out and it keeps its corner radius.
- **Swaps:** grow or shrink with `scale()`, move with `translate()`, show and hide with `opacity`,
  and animate shadows as `filter: drop-shadow()`, since `box-shadow` repaints every frame.
- **Never `transition: all`.** It animates whatever changes next, layout included. Name the
  properties.
- **`translate` percentages are relative to the element itself.** `translateY(100%)` moves an element
  by its own height whatever its content. Prefer them to hard-coded pixels.
- **`transform-origin` at the trigger** for popovers, menus and tooltips (Base UI's
  `var(--transform-origin)`). Modals are not anchored, so they stay centred. `scale()` scales the
  children too, which is what makes a press read as physical.
- **Where it runs.** A JS `requestAnimationFrame` loop always runs on the main thread and drops frames
  while the page loads, hydrates or runs scripts. CSS and WAAPI can run off it. Motion hands
  `opacity`, `transform`, `clip-path` and `filter` to WAAPI. Its `x`/`y`/`scale` shorthands are
  accelerated from v14.2 (before that, animate the full `transform` string). Layout animations
  (`layout`, `layoutId`), `onUpdate`, `transformTemplate`, inertia and values driven every frame
  stay on the main thread in every version. Vercel's tab indicator dropped frames as a shared-layout
  animation during page loads and was smooth once moved to CSS.
- **Don't animate through component state.** A `setState` per frame re-renders every frame. Write to
  `ref.current.style.transform` or a motion value.
- **Don't drive a child's transform from a CSS variable on its parent.** Variables inherit, so every
  descendant restyles each frame (Vaul's drag slowed once a drawer held about 20 items). Set
  `transform` on the element.
- **Blur:** keep animated `blur()` under 20 px (2–5 px is enough to soften a crossfade). Heavy blur
  is expensive, worst in Safari. Never animate the intensity of a native blur view.
- **`will-change: transform` comes last,** and only on the animating element once you have seen a
  1 px shift at the start or end of a motion or measured dropped frames. Every promoted layer costs
  GPU memory.
- **GTK:** tick callbacks, not timeouts, and no idle animation (`perf-gtk`). **Compose:**
  `graphicsLayer` transforms and deferred state reads (`perf-android`).

**When something janks**, look for the cause in this order and name it before changing code: a
non-composite property (layout first, then shadows and radius), motion that runs while the page is busy,
a re-render per frame, cost that grows with content (an inherited variable, too many animating
nodes), old Motion shorthands or an animated variable, blur over 20 px, and only then `will-change`.
Confirm it with a recording (the DevTools Performance panel, or Perfetto on Android) on a real
mid-range device. CPU throttling on a desktop doesn't model a weak GPU. Report each finding as the
element and `file:line`, the cause, before, after, and whether it was verified from code or still
needs a device recording.

## 5. Interruption and gestures

The interface feels alive when motion starts from where things are now, inherits the finger's
speed, carries momentum forward, and can be grabbed and reversed at any moment.

- **Respond on press-down and commit on release.** Waiting for `click` or touch-up feels dead. Keep
  feedback continuous during a drag, slider or sheet: update 1:1 with the pointer the whole way, not
  only at the end.
- **Never lock out input during a transition.** A closing sheet that the person grabs again follows
  the finger.
- **Interrupt from the presentation value.** Read the live on-screen transform and start from it.
  Starting from the target value makes a visible jump.
- **Transitions, not keyframes, for anything fired rapidly** (toasts, toggles): transitions retarget
  from the current value, keyframes restart from zero. **Springs for gestures,** because they carry
  velocity. On a reversal, blend the velocity instead of cutting it. Drive 2D motion with separate X
  and Y springs.
- **Track 1:1 from where it was grabbed.** Keep the grab offset (never snap the element's centre to
  the pointer), `setPointerCapture` once a drag starts, keep a short position and time history for
  velocity, and ignore extra touch points once a drag is under way (`if (isDragging) return`).
- **Hysteresis:** about 10 px of movement before committing to a direction, and about 10 px of slop
  around a tap target, so the person can cancel by dragging away and back.
- **Recognise all plausible gestures from the first move** and drop the losers once intent is clear.
  Pay the double-tap delay only where double-tap exists.
- **Velocity handoff.** On release the spring continues at the finger's velocity. Motion and
  Reanimated take px/s directly; an API that wants relative velocity takes
  `velocity / (target − current)`.
- **Throw to where it's going.** Project the resting point, then snap to the target nearest the
  projection. Decide between dismiss and return by the velocity's sign, not the position.

  ```js
  // Apple's projection; decelerationRate 0.998 for normal scroll feel, 0.99 for snappier
  const project = (v /* px/s */, d = 0.998) => (v / 1000) * d / (1 - d);
  const target = nearestSnapPoint(position + project(releaseVelocity));
  ```

- **A flick is enough.** Dismiss when the distance passes the threshold **or** the velocity
  (`|distance| / elapsed ms`) passes about 0.11.
- **Rubber-band at edges** instead of a hard stop. Resistance grows the further past the edge:

  ```js
  const rubberband = (over, size, c = 0.55) => (over * size * c) / (size + c * Math.abs(over));
  ```

- **Spatial consistency.** Exit the way it entered: a toast that rose from the bottom leaves through
  the bottom. Originate from the trigger. Let in-between frames point at the outcome, the way a
  control grows toward the finger.
- **Pointer-following decoration** (a tilt that tracks the mouse) goes through a spring rather than
  the raw position. It belongs on marketing surfaces only.

## 6. Haptics and sound

- **Causality:** fire on the causal event (the detent catching, the toggle flipping) with a character
  that matches the act.
- **Harmony:** the visual, the haptic and the sound land on the same frame. A lag reads as a glitch.
- **Utility:** one per action, only at meaningful moments (commit, snap, success, error). Never on
  scroll, per frame, or on an entrance the person didn't cause. Never the only feedback: haptics are
  off for many people and silent on many Android phones.

## 7. Reduced motion and accessibility

Reduced motion ships with the animation, not later, and means fewer and gentler, not none.

- `prefers-reduced-motion: reduce` (Compose and GTK read the system setting): replace slides,
  springs, parallax and overshoot with short opacity cross-fades. Keep opacity and colour changes
  that explain a state. Screen transitions become fades. Everything still settles to its end state.
- `prefers-reduced-transparency`: frostier or solid surfaces, no blur. `prefers-contrast: more`:
  near-solid backgrounds with a defined border.
- Gate hover motion behind `@media (hover: hover) and (pointer: fine)`, because touch fires false
  hovers on tap. Hover is a colour or surface step, never a scale or lift.
- Avoid full-viewport moving backgrounds, slow loops near 0.2 Hz, and sudden brightness jumps (ease a
  theme change). Fade large surfaces while they travel a long way.
- Text scales, so never animate to a height measured at the default type size. Measure it, or
  animate a transform.

```css
@media (prefers-reduced-motion: reduce) {
  .sheet { transition: opacity 200ms ease; transform: none; }
}
```

## 8. Feel-check

Code can't settle feel. When a result depends on it (a crossfade, a spring's bounce, opacity against
height in a list that grows), say so and name the check:

- Play it at 2–5× the duration, or at 10% in the DevTools Animations panel, and step frame by frame.
  Look for two states overlapping in a crossfade, abrupt starts or stops, a wrong origin, and
  properties drifting out of sync.
- Judge gestures, velocity handoff and haptic timing on a real device, in a release build (Expo Go,
  the simulator and desktop emulation all hide the problems): flick it, interrupt it mid-flight,
  reverse it.
- Look again the next day.

## 9. Never ship

| Never | Instead |
| --- | --- |
| Motion on a keyboard action or a 100+/day surface; sliding tabs | None |
| `transition: all` | The exact properties |
| `scale(0)` or a fade with no transform on entry | `scale(0.95)` + `opacity: 0` |
| `ease-in` on UI; a weak built-in curve on deliberate motion | `--ease-out` / `--ease-in-out` |
| UI motion over 300 ms with no reason | 150–250 ms |
| `transform-origin: center` on a trigger-anchored popover | The trigger's origin (modals exempt) |
| Keyframes on toasts, toggles or anything fired rapidly | Transitions or springs |
| Animated layout properties | `transform` / `opacity` |
| A parent CSS variable driving a child's transform; `setState` per frame | Write `transform` on the element |
| Ungated `:hover` motion; hover scale or lift | Gated colour or surface change |
| No reduced-motion handling, or reduced motion that removes all feedback | A gentler variant |
| The same speed for press and release | A slow deliberate phase, a quick response |
| Everything entering at once, or a stagger over 300 ms total | 30–60 ms stagger, or one group fade |
| Idle loops (pulse, shimmer, glow, float) on a working screen | Static; a loop only for something actually live or loading |
| A spinner or shimmer on something that resolves under 300 ms | Nothing |
| A distance-only dismiss; a hard stop at an edge | Velocity or distance; rubber-banding |
| Bounce on something that wasn't thrown | Damping 1.0 |
