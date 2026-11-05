---
name: house-style
description: The owner's design package for any interface (desktop, phone, web, game UI, mockups) - their taste (clean, professional, near-black with one accent, nothing crowded, too round, too big, boxed, dated or AI-looking), how to work with them, the fundamentals of hierarchy, type, spacing, colour, controls, states, copy, data and icons, and task guides - motion (whether to animate, curves, durations, springs, gestures, haptics, reduced motion, jank; reviewing, auditing, finding or removing animations; naming an effect; React Native), unslopping an AI-looking screen, landing heroes and generated visuals, worst-case data stress tests, web apps that feel native on a phone, React library picks and variations in code. Load before designing, redesigning, animating, polishing or reviewing any screen, component or artboard.
---

# House style

This is how the owner (Antho) wants their interfaces to look and work. It was learned from what they
chose, rejected and corrected across Relay (GTK desktop and Android), Avex (Android gym coach) and
Tally (Android budget). Read `my-list.md` beside this file first: the owner's own rules, which
they keep by hand. Then read this skill, follow the platform skill (`house-style-native`) for the
stack you are on, and run `house-style-review` before you call it done.

## Who wins

1. **What they say in this conversation.** A named reference, colour or layout beats any rule here.
2. **`my-list.md`, the owner's list.** They edit it by hand. It beats every other file in this
   package and the project's doc; where a rule below or in `reference/` says otherwise, the list
   wins and you don't argue it from `evidence.md`. Never edit it unless they ask.
3. **The project's own design doc** (`DESIGN.md`, `.claude/DESIGN.md`, `PRODUCT.md` and its
   satellites). It holds the exact tokens, components and settled decisions. Read it first, use its
   names, and never re-add something its removals ledger lists (`SETTLED.md`, `FAILURES.md`).
4. **This skill.** Their taste, where the project doc is silent.
5. **Generic design skills** (impeccable and the like) and your own instincts. Where they push for
   "bold", "out-of-distribution" or expressive treatment of an app screen, this skill wins: their apps
   are tools they use every day. Their craft checks still apply. Use their bigger moves only on
   surfaces meant to persuade or show off: a launch beat, a release video, a landing page.

## The look, in one paragraph

Clean and professional, like Claude's apps or t3code: a near-black ground, neutral or slightly warm,
off-white ink, **one accent used for what asks for you**, state colours only where they mean state,
and real colour only in data (charts, categories). Dense where it is a tool, never crowded. Flat
tonal depth, not shadows, glows or gradients. Moderate corners, not round everything. Trim
controls, not chunky ones. Every element is there because it changes what they do, not "because
it needs to be there". The same look across all their apps and spaces: a new page or app wears the
existing shell, it does not invent its own world.

## What they reject (each has happened, most more than once)

| They say | It means | So |
|---|---|---|
| "dating", "doesn't feel pro", "outdated" | Hairlines between everything, boxes around passive content, stock Material/Adwaita surfaces, unicode glyphs as icons, heavy borders, uneven spacing | Flat tonal surfaces, air instead of rules, the project's own controls and drawn icons |
| "AI looking", "crowded", "everything looks like it's there to be there" | Uniform lists of status rows, stat-card walls, a caption under every heading, too many sections | One mark plus the one detail that matters; cut sections with no decision behind them |
| "too round", "full of rounded everywhere", "rounded grey backdrop" | Pills and big radii on everything, grey rounded boxes behind content | Pills only for small toggles, badges and counts; keys 8–10 px; passive content gets no backdrop |
| "too big", "too drawn out", "no reason to be this big" | Oversized popups, buttons, figures, panels, long notification text | Trim sizes; one line where one line works; scale a control with its section, not alone |
| "too futuristic", "too see-through", "stands out too much, full of red" | Neon, glow, sci-fi frames, inconsistent translucency, accent spent everywhere | One translucency scale for the whole app; accent in few places |
| "the text almost goes out the box", "doesn't adjust to the text" | Fixed heights and widths around text | Containers hug content; panels resize with what they hold |
| "you scroll down too much" | Low-value sections stacked | Put the answer above the fold, cut or move the rest |
| "feels empty" | A page with nothing useful on it | Add real readings (stats, counts, recent items), never filler |
| "I hate the font, put back the one before" | A face swapped without asking | Never change fonts unasked |
| "the - when a day is skipped is bad" | Machine placeholders (dashes, n/a, raw ids) | Draw the state (a rest-day icon, an honest zero) |

## What they ask for

- **Graphs and readings.** "I basically want graphs and info." Show the numbers behind a verdict,
  and do not force charts into black and white: data may be colourful.
- **The idea kept, the UI redone.** "I like the idea not the UI and UX": keep the flow and the
  features, rebuild the visuals. "I quite liked the old design, improve the old one instead": when
  they liked the old version, refine it. "Cleaned up a bit" means refine, not redesign.
- **One app, not worlds apart.** Threads looks like Dev, Tally wears Relay's phone look, Settings
  matches the rest of the app ("the settings look nice but too different from the rest").
- **Platform-right navigation.** Back arrows at the top left of a page, a notification bell with a
  count, Settings and accounts shared across spaces.
- **Pro-quality icons.** A custom set on one grid and one stroke (Relay: 16-unit grid, 1.5–1.75),
  optically centred in their keys. Toolbar labels go under the icon, not beside it.
- **Motion with intent.** Startup and intro beats may show off ("like a motion design showcase but
  not too much, keep it short"), stagger related items, and can respond to the pointer. Popups
  animate in smoothly. Working screens do not idle-animate.
- **Customisation where it's cheap.** They asked for accent and base-colour settings. Build colour
  through tokens so a palette swap is one change.
- **Things where they'd look for them.** Put a control on the page it acts on, a page in the section
  they would open. Notices go behind the bell, not in a strip above the page.
- **Familiar references.** They name them: Claude's desktop and phone apps, t3code, KWrite for a text
  editor, GitHub's issue page for a task page, Claude's design tool for the canvas. When they name
  one, study its layout and conventions, then express it in the project's look.

## Rules they set that hold everywhere

- **One answer per screen, above the fold.** Nothing (notice, tip, strip) sits between the page's
  answer and its main action.
- **One filled primary action per surface.** Row actions are drawn outlined pills with the whole row
  as the tap; five filled buttons is a "button wall".
- **Surfaces are earned by interaction** on phone apps: if you can't tap it, it gets no box.
  Desktop Relay groups rows on glass cards by its own Glass Rule; follow the project doc.
- **Lines are data,** never decoration (chart axes, meter tracks, table rules).
- **Show the reading, not just the verdict.** "Over pace" travels with the amount and the days.
- **Honest at zero.** Every empty state is drawn with the section's own shape, never hidden and never
  a row of "No data".
- **Undo over confirm.** Confirm only what cannot be undone, naming the act.
- **Voice: dry, specific, their own numbers.** No exclamation marks, no em dashes, no hype, no
  praise the data doesn't support, no explaining the obvious ("cringy"). Speak in "you". Machine
  identifiers never render.
- **Contrast is measured on the pixels:** text ≥ 4.5:1, meaningful marks ≥ 3:1. Filled vs empty
  differs by shape, not only by brightness.
- **Leave alone what they didn't name.** "The rest is fine" means the rest is fine. In Relay the
  terminal panes are never restyled unless they ask.

## How to work with them

1. **Read first:** the project's design doc and removals ledger, then the screen as it is today (a
   screenshot or render, not just the code).
2. **For a redesign, make one version** unless they ask for options. It must really differ from the
   before ("you didn't change much" is a failure). When they ask for options, make 2–3 that differ
   in layout, hierarchy or density, not three colourways of one layout, named A, B, C so they can
   answer "B, but take C's chart". Put **the before** beside them. On a Relay project mockups go
   on the design canvas (load `design-canvas`), one artboard each.
3. **One screen at a time, overview first.** Finish and agree the main page before its sub-pages,
   then reuse the agreed style for the rest ("after we can use this style format for the rest of the
   app").
4. **Look before you report.** Render or screenshot the result at real size, check it against
   `house-style-review`, fix what you see, then show them. A web page is shown as the real thing in
   a browser on localhost, checked at every width first. They judge on the device, and things that
   won on argument have been cut on sight.
5. **Decide, don't ask, when they say so.** "Don't ask me, take care of the design" means make the
   taste calls yourself with this skill and say what you chose. Otherwise flag a real taste call
   (a debatable placement, a removal) in one line and keep going.
6. **Write decisions down.** A choice they make or reverse goes into the project's design doc the same
   turn (or its settled/decisions satellite), so the next agent doesn't undo it.
7. **Small batches of feedback come as numbered lists.** Answer each number, fix each one, and don't
   drop the small ones ("skill icon is not centred").
8. **A correction keeps their placement.** Fix only what they named and ask nothing they already
   said. If the correction is a rule worth keeping, offer it as one line for `my-list.md` in your
   reply; they add it themselves.

## What to read next

**First, the kind of surface.** It decides how dense the screen is, how much moves, and where colour
may go.

| Surface | Examples | Motion | Colour and visuals |
|---|---|---|---|
| Working screen, used daily | Board, terminals, ledger, coach list, Git panel | Feedback only. Nothing idles, nothing on keyboard actions | Neutral chrome, colour in data |
| Occasional | Settings, sheets, dialogs, pickers | Standard | The same |
| Rare moment | First run, empty state, success, a startup beat | The delight budget | One moment may be rich |
| Persuasion | Landing page, release video, store listing | Expressive, short, once | A scene or product visual (`landing-visuals.md`) |

**Then the task.** Every file is in `reference/` and loads only when the task needs it.

| Task | Read |
|---|---|
| Any new screen or component | `fundamentals.md` (hierarchy, type, spacing, colour, contrast, controls, the seven states, copy, data, icons, accessibility), then `house-style-native` |
| "Something's off and I can't point at what" | `failures.md`, the named failure shapes with fixes |
| Animate anything: transition, gesture, drag, sheet, spring, haptics, reduced motion, jank | `motion.md`, plus `motion-recipes.md` for web code or `react-native.md` for Expo |
| Review a diff's motion, audit and plan, find what should move, remove what shouldn't | `motion-passes.md` |
| "What's it called when…" a motion effect | `motion-vocabulary.md` |
| A screen that looks AI-made, or a polish or clean-up pass | `unslop.md` |
| A landing hero, a generated image, the unslop story video | `landing-visuals.md` |
| Worst-case data and edge cases | `stress-test.md` |
| A web app or PWA used on a phone | `mobile-web.md` |
| Picking a React library | `libraries.md` |
| Variations built in code | `variations.md` |
| Why a rule exists, in the owner's words with dates | `evidence.md`. Check it before arguing a rule doesn't apply |
| Before calling it done | `house-style-review` |

This package replaces the separate generic design skills (emil-design-eng, apple-design, unslop,
the animation skills, break-ui, mobile-native, pick-ui-library, prototype). If any of them is still
installed, these files win wherever they differ.
