---
name: house-style-native
description: The owner's house style applied per platform - GTK 4 desktop apps in Rust (Relay - theme.css tokens, the Glass Rule, native controls, keyboard and focus, title-bar folding, headless screenshots in a virtual KWin), Android with Jetpack Compose and Material 3 (Avex, Tally, Relay's phone app - theme tokens instead of stock Material, typography from the theme, 200% font scale, 44-48 dp targets, TalkBack descriptions on Canvas marks, Roborazzi and golden screenshots), web pages and HTML artifacts (tokens on :root, dark and light, phone width), design-canvas mockups, and game UI (Unreal, Unity, Godot HUDs and menus). Load with house-style when building or changing UI on one of these stacks.
---

# House style on each platform

`house-style` says what the owner wants. This says how that lands on each stack. On every stack the
project's own design doc names the exact tokens and components; use those names, never raw values.

## GTK 4 desktop (Relay, `apps/relay-native`)

- **Tokens live in `src/theme.css` and per-mode palettes in `fonts::PALETTES`.** Ground `#070707`,
  pages `#0f0f0f`, ink `#ededed`, one coral accent `@signal` for what asks for you, `live` green,
  `held` red, `waiting` amber. Never a hex in a widget's CSS.
- **The Glass Rule:** what sits on a page is a veil of `@ink` at one of five strengths by role
  (well 3%, card 4%, control 5.5%, hover 7.5%, selected 9%), with glass edges at 7/11/18%. Only what
  floats over content (menus, popovers, sheets) is opaque, using the `_solid` twins. A new page
  uses these classes; it never invents a translucency.
- **Shapes:** keys 10 px, icon keys 7 px, popovers 12 px, cards and sheets 14 px, pills for small
  toggles and badges only. Terminal plates are square, and **the terminal panes are never restyled**
  (plates, identity strips, lamps, Fira faces, gutters, `@screen`) unless the owner asks.
- **Type:** Geist 13 px controls, 14 px body, 12 px captions; Geist Mono 11.5 px for figures,
  keycaps, paths, the status bar; Sora only for the wordmark. Weights 400/500/600.
- **Keys:** 28 px minimum (34 in the title bar), primary is ink with ground text, quiet keys are
  transparent. Focus is a 2 px ink ring at 35%. Disabled is 40% opacity.
- **Shell:** one title bar (brand over the sidebar, then the project's strip, Design/Files/Git keys
  on the right, the bell with a count), the sidebar with Dev | Threads on top, the status bar with
  the resource key. No search or inbox in the sidebar. Pages are cards inset 8 px from the window
  edge. New spaces (Threads, Aide) wear this shell, not their own.
- **Narrow windows fold, never clip:** names ellipsize, then labelled keys drop to icons, then
  utility keys go into a More menu (`fold.rs`). Test at the minimum width.
- **Native first:** GTK controls, GtkSourceView and VTE keep their own keyboard, selection, clipboard
  and accessibility. Don't copy web layout rules into GTK without checking native behaviour. Escape
  closes sheets, Enter or Ctrl Enter commits, and every action has a keyboard path.
- **Hidden widgets keep stale named colours after an appearance reload:** check every mode (Matte,
  Dark, OLED, and a white wallpaper) after a palette change.
- **See it:** screenshots only inside a virtual compositor (`kwin_wayland --virtual
  --no-lockscreen`), never on the owner's desktop; `scripts/native-smoke.py` and
  `RELAY_NATIVE_SCREENSHOT` captures. Mute anything that plays sound. If the stack can't be built in
  the session, say so plainly; reading CSS is not a render.
- **Performance is part of the look:** no idle animation, no default-priority future floods, nothing
  slow on the main loop (load `perf-gtk` when a change touches paint or layout cost).

## Android, Jetpack Compose (Avex, Tally, Relay's phone app)

- **Theme, not stock Material.** Map the project's palette onto `MaterialTheme.colorScheme`,
  including the whole `surfaceContainer*` ladder, and switch off the tonal tint so sheets and menus
  don't arrive in Material's purple-grey. Colours come from `colorScheme`, never raw values.
- **Each app's look:**
  - Relay's phone app and Tally 2 share Relay's phone tokens: wall `#171614`, ink `#EDE9E2`, keys
    10 dp, cards 14 dp, Geist / Geist Mono / Sora, Relay's glyphs.
  - Avex is "open editorial": warm near-black Pearl, a user-picked accent (Red default) with a
    monochrome option, three type voices (serif figures, sans prose, mono small-caps labels).
  - Read the app's doctrine file before touching it (`.claude/DESIGN.md` in Avex, `DESIGN.md` in Tally).
- **Type from the theme only.** No `fontSize =` at a call site. Use tabular numerals for amounts
  and counts. One hero figure per screen.
- **Font scale 200% must survive:** no fixed `.height()` on anything holding text, `heightIn(min)`
  instead; no `maxLines = 1` on user content; figure rows wrap. Check every touched screen at 100%
  and 200%.
- **Touch:** 48 dp targets from padding (Tally: 44 dp minimum controls), one target per row, row
  actions as drawn outlined pills. One filled primary per page; a modal's single commit may take
  the accent.
- **Navigation:** top bar is the back arrow plus at most one action, never the screen's own name
  (settings pages may use a collapsing large title). Lens/tab switches are segmented pills. Notices
  go to the bell's feed. Undo is a snackbar; dialogs only for what can't be undone.
- **States:** local Room data is instant, so no spinners or skeletons over it. Shimmer only for real
  latency (photos, Health Connect, network).
- **Motion:** shared durations and springs from the project's `Motion.kt` (no literal durations),
  press feedback per the app (Avex: bounce, no ripple, ripple restored under TalkBack), and
  reduce-motion settles everything.
- **Semantics:** every Canvas mark carries a `contentDescription` reading its value ("4 of 5
  sessions this week"); start/end, never left/right.
- **Icons:** the app's custom families on one grid and stroke; Material stock only where the project
  allows it (Tally: Material Rounded for category icons, because their names are synced data).
- **See it:** golden screenshots (Avex `RecipeScreenshotTest`, Relay `ScreensTest` with Roborazzi:
  `-Proborazzi.test.record=true`; captures can land before seeded data loads, so re-record before
  trusting a diff) or the emulator. Never force-stop or relaunch apps on the owner's phone once a
  step is theirs.

## Web pages and HTML artifacts

- Colours as custom properties on `:root`, redefined for dark mode; an explicit `body` background.
  Dark is the owner's default look: near-black ground, off-white ink, one accent.
- System or a bundled sans (Geist if available) plus a mono for figures; one display face at most.
- Works at 360 px wide with a 16 px gutter and no horizontal scroll; content column capped near
  720 px for reading, wider grids for tools.
- Same rules as the apps: one primary, no boxes around passive content, lines as data, honest zeros,
  charts open on the page and allowed real colour.

## Design-canvas mockups (Relay's Design view)

- Load `design-canvas` for the mechanics. Copy the design's shell template artboard for a new Relay
  screen (designs 12 and 13 hold the approved A2 "Workbench, reworked" look).
- Build with auto layout, named colours and styles that match the project's tokens, real copy,
  an 8 px rhythm. Variations go side by side, 80 px apart, named A/B/C, with the before beside them.
- `design.render` every artboard and read its `issues` before showing the owner.

## Game UI (Unreal, Unity, Godot)

- Match the game's art direction first: the owner asked for "a distinct style and atmosphere", so
  there is no Relay chrome in a game. The craft rules still hold: one focal point per screen,
  readable at couch distance, nothing crowded, honest states.
- HUDs: show only what changes a decision mid-play, and keep the screen centre clear. Menus are
  trim, keyboard- and controller-navigable, with the selected item obvious.
- Text: dry and specific, no placeholder copy. Check overlap and clipping at every resolution the
  game ships (and 16:9 plus ultrawide).
