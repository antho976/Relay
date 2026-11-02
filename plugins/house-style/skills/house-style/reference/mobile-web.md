# A web app that feels native on a phone

The small platform fixes that separate "a website in a browser" from something that feels installed.
Most "it feels janky on my phone" reports aren't motion problems: they're a tap delay, a grey flash
or a hover that won't let go. Phone apps built natively follow `house-style-native` instead.

- **Media queries, not device sniffing:** `(hover)`, `(pointer)`, `env()` and `dvh` say what the
  device can do. Touch and mouse coexist (iPads with trackpads, touch laptops), so gate by
  capability.
- **Apply each fix where its reason applies.** `user-select: none` on a button is right, and on body
  text it's a defect.
- **Never disable zoom.** `user-scalable=no` and `maximum-scale=1` are accessibility failures. Fix the
  input font size, which was the cause.
- **Real hardware decides.** None of this reproduces in desktop device emulation. Run the dev server
  on `0.0.0.0`, open it by LAN IP, and inspect with Safari's Develop menu or `chrome://inspect`. Test
  on a phone a few years old, with the keyboard open, once in landscape, and as an installed PWA if
  that's a target. Say which fixes you verified from code and which need a phone.

| Symptom | Cause | Fix |
| --- | --- | --- |
| Hover state stuck after a tap | Touch fakes `:hover` on first tap | Every hover rule inside `@media (hover: hover) and (pointer: fine)`; touch gets `:active` |
| Grey or blue flash on tap | The browser's tap highlight | `-webkit-tap-highlight-color: transparent` on `html`, then give every tappable its own `:active` |
| Wrong height, bottom button under the URL bar | `100vh` is the largest viewport | `100dvh` for app shells and drawers; `100svh` (min-height) for heroes, which never shifts |
| Page zooms into an input and stays zoomed | iOS zooms under 16 px | Inputs at 16 px minimum (`@media (pointer: coarse)` if desktop wants smaller) |
| Tap feels laggy | The double-tap wait, plus feedback on release | `touch-action: manipulation` on controls; style `:active` or listen to `pointerdown`, not `click` |
| Pull-to-refresh or page bounce hijacks scroll | Overscroll chains to the page | `overscroll-behavior: none` on `html, body`; `contain` on inner scrollers. Never `touchmove` + `preventDefault` |
| Content stops short of the notch | Letterboxed safe area | `viewport-fit=cover`, then pad with `env(safe-area-inset-*, 0px)` on fixed headers, bottom bars, toasts and sheets |
| Long-press selects a button's label | Controls are selectable text | `user-select: none; -webkit-touch-callout: none` on controls only |
| Carousel drags the page vertically | The browser guesses the axis | `touch-action: pan-y` on a horizontal gesture surface (`none` only where the element owns every axis), or native `scroll-snap-type: x mandatory` |
| Status bar colour clashes | One `theme-color` for both schemes | One `theme-color` per `prefers-color-scheme`, matching the header colour (update it on a manual theme toggle) |

Set the keyboard while in the inputs: `inputmode="numeric"` for codes, `"decimal"` for amounts,
`type="email"`/`"tel"`, `autocapitalize="none"` and `autocorrect="off"` on usernames and codes, and
`enterkeyhint="send" | "search" | "done"`.

## Baseline for a phone-facing app

```html
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover, interactive-widget=resizes-content" />
<meta name="theme-color" media="(prefers-color-scheme: light)" content="#ffffff" />
<meta name="theme-color" media="(prefers-color-scheme: dark)" content="#0a0a0a" />
<meta name="color-scheme" content="light dark" />
```

```css
html {
  -webkit-tap-highlight-color: transparent;
  -webkit-text-size-adjust: 100%;
  overscroll-behavior: none; /* drop this for a scrolling document where pull-to-refresh is welcome */
}
input, textarea, select { font-size: 16px; }
button, a, [role="button"] {
  touch-action: manipulation;
  user-select: none;
  -webkit-user-select: none;
}
@media (hover: hover) and (pointer: fine) {
  /* every :hover rule lives here */
}
```

`interactive-widget=resizes-content` makes Android Chrome's keyboard shrink the layout viewport like
iOS does, so `100dvh` and bottom-pinned inputs follow it. In Next.js the theme colours go through the
`viewport` export. For a PWA, the manifest's `theme_color` and `background_color` and
`apple-mobile-web-app-status-bar-style` are the same decision.
