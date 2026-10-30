---
name: house-style
description: The owner's design taste and the craft rules under it, for any interface they build (desktop, phone, web, game UI) - what they ask for and what they reject, learned from their Relay, Avex and Tally work (clean, professional, black with at most a little colour, nothing crowded, too round, too big, boxed, dated or AI-looking), precedence over generic design skills, how to work with them (variations side by side with the before, one screen at a time, keep what they like, look at the render before reporting), and the fundamentals of hierarchy, type, spacing, colour, contrast, controls, states, copy, motion, data and icons. Load before designing, redesigning, polishing or reviewing any screen, component, mockup or design-canvas artboard.
---

# House style

This is how the owner (Antho) wants their interfaces to look and work. It was learned from what they
chose, rejected and corrected across Relay (GTK desktop and Android), Avex (Android gym coach) and
Tally (Android budget). Read it before any UI work, then follow the platform skill
(`house-style-native`) for the stack you are on and `house-style-review` before you call it done.

## Who wins

1. **What they say in this conversation.** A named reference, colour or layout beats any rule here.
2. **The project's own design doc** (`DESIGN.md`, `.claude/DESIGN.md`, `PRODUCT.md` and its
   satellites). It holds the exact tokens, components and settled decisions. Read it first, use its
   names, and never re-add something its removals ledger lists (`SETTLED.md`, `FAILURES.md`).
3. **This skill.** Their taste, where the project doc is silent.
4. **Generic design skills** (impeccable and the like) and your own instincts. Where they push for
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
2. **For a redesign, offer 2–3 variations that really differ** (layout, hierarchy, density), not
   three colourways of one layout. "You didn't change much" is a failure. Put **the before** beside
   them. On a Relay project draw them on the design canvas (load `design-canvas`), one artboard each,
   named A, B, C, so they can answer "B, but take C's chart".
3. **One screen at a time, overview first.** Finish and agree the main page before its sub-pages,
   then reuse the agreed style for the rest ("after we can use this style format for the rest of the
   app").
4. **Look before you report.** Render or screenshot the result at real size, check it against
   `house-style-review`, fix what you see, then show them. They judge on the device, and things that
   won on argument have been cut on sight.
5. **Decide, don't ask, when they say so.** "Don't ask me, take care of the design" means make the
   taste calls yourself with this skill and say what you chose. Otherwise flag a real taste call
   (a debatable placement, a removal) in one line and keep going.
6. **Write decisions down.** A choice they make or reverse goes into the project's design doc the same
   turn (or its settled/decisions satellite), so the next agent doesn't undo it.
7. **Small batches of feedback come as numbered lists.** Answer each number, fix each one, and don't
   drop the small ones ("skill icon is not centred").

## Fundamentals

The general craft (hierarchy, type, spacing, colour, contrast, controls, the seven states, copy,
motion, data display, icons, accessibility) is in `reference/fundamentals.md`. Read it for any new
screen. The named failure shapes, each with its fix, are in `reference/failures.md`: use them to
diagnose "something's wrong and I can't point what". Their own words behind each rule, with dates,
are in `reference/evidence.md`. Check there before arguing that a rule doesn't apply.
