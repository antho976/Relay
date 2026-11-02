# Motion recipes (web)

Starting points for the common cases. Values and rules come from `motion.md`; the curves are its
`--ease-out`, `--ease-in-out` and `--ease-drawer` tokens. Each recipe still ships with reduced motion
and gated hover (`motion.md` §7). Native equivalents: Compose and GTK use the project's own tokens,
React Native is in `react-native.md`.

## Press

```css
.button { transition: transform 160ms var(--ease-out); }
.button:active { transform: scale(0.97); }
```

`:active` is a real press on touch as well, so it needs no hover gate.

## Popover, dropdown, menu, select

```css
.popover {
  transform-origin: var(--transform-origin); /* Base UI supplies it */
  transition: opacity 200ms var(--ease-out), transform 200ms var(--ease-out);
}
.popover[data-starting-style],
.popover[data-ending-style] { opacity: 0; transform: scale(0.95); }
```

## Tooltip

The popover at 125 ms and `scale(0.97)`. After the first tooltip in a toolbar has opened, its
neighbours open with no delay and no animation:

```css
.tooltip[data-instant] { transition-duration: 0ms; }
```

## Modal

The popover with `transform-origin: center`, 250 ms, from `scale(0.96)`. Its backdrop fades on the
same timing, so the two read as one surface.

## Drawer, sheet

```css
.drawer { transform: translateY(0); transition: transform 400ms var(--ease-drawer); }
.drawer[data-closed] { transform: translateY(100%); }
```

Once it can be dragged, it is a gesture: see "Drag to dismiss".

## Toast

```css
.toast {
  transition: opacity 400ms ease, transform 400ms ease;
  @starting-style { opacity: 0; transform: translateY(100%); }
}
```

Sonner uses `ease` and runs a little slower than the usual UI budget on purpose, because that suits
the component. When toasts stack and the list reflows, balancing opacity against height is trial and
error. Tune it, then look again the next day.

## Accordion

```css
.content { overflow: hidden; transition: height 200ms var(--ease-out), opacity 200ms var(--ease-out); }
```

Animate to a measured height (or one a headless primitive supplies), never to `auto`, and keep it
short, since it pays for layout every frame.

## Group entrance

```css
.item { opacity: 0; transform: translateY(8px); animation: rise 300ms var(--ease-out) forwards; }
.item:nth-child(2) { animation-delay: 50ms; }
.item:nth-child(3) { animation-delay: 100ms; }
@keyframes rise { to { opacity: 1; transform: translateY(0); } }
```

## Hold to confirm

```css
.overlay { clip-path: inset(0 100% 0 0); transition: clip-path 200ms var(--ease-out); }  /* release */
.button:active .overlay { clip-path: inset(0 0 0 0); transition: clip-path 2s linear; }  /* press */
.button:active { transform: scale(0.97); }
```

`linear` is right here: the fill is progress, and progress doesn't ease.

## Tabs with an exact colour change

Duplicate the tab list and style the copy as active (its own background and text colour). Clip the
copy so only the active tab shows, and animate the clip:

```css
.tabs-active-copy { clip-path: inset(0 60% 0 20%); transition: clip-path 250ms var(--ease-in-out); }
```

Text and background change in sync because one element is revealed, rather than two colours being
interpolated.

## Clip-path reveals

`clip-path: inset(top right bottom left)` eats in from each side.

- **Scroll reveal** (marketing pages only, once): from `inset(0 0 100% 0)` to `inset(0 0 0 0)` over
  600 ms `--ease-in-out`, triggered by `IntersectionObserver` or Motion's
  `useInView({ once: true, margin: "-100px" })`.
- **Before/after slider:** two images stacked, the top one at `inset(0 50% 0 0)`, with the right
  inset following the drag. No extra elements, and fully composited.

## Drag to dismiss

```js
const velocity = Math.abs(swipe) / (Date.now() - dragStart);
if (Math.abs(swipe) >= THRESHOLD || velocity > 0.11) dismiss();
element.style.transform = `translateY(${distance}px)`; // on the element, not through a parent variable
```

Add pointer capture, multi-touch protection and rubber-banding past the edge (`motion.md` §5), and
settle with the release spring and its velocity.

## Masking a crossfade that won't settle

When two states visibly overlap whatever the easing and duration, blur the seam so the eye reads one
change:

```css
.content { transition: filter 200ms ease, opacity 200ms ease; }
.content.transitioning { filter: blur(2px); opacity: 0.7; }
```

## Programmatic, no library

```js
element.animate(
  [{ clipPath: 'inset(0 0 100% 0)' }, { clipPath: 'inset(0 0 0 0)' }],
  { duration: 1000, fill: 'forwards', easing: 'cubic-bezier(0.77, 0, 0.175, 1)' }
);
```

## Spring in Motion, reduced motion in React

```jsx
animate(el, { y: 0 }, { type: 'spring', bounce: 0, duration: 0.4 });            // default settle
const rotation = useSpring(mouseX * 0.1, { stiffness: 100, damping: 10 });      // decorative pointer tilt
const reduce = useReducedMotion();
const closedX = reduce ? 0 : '-100%';
```

## Depth in CSS

`rotateX`/`rotateY` with `transform-style: preserve-3d` give flips, orbits and depth without JS.
Keep them to showcase surfaces.

## Glass surfaces arriving

For a blurred or glass surface, animate blur radius and scale together on enter and exit, so it
reads as a material arriving rather than a plain fade.
