# Forge — doctrine decision log

Satellite of `.claude/DESIGN.md`. Why the doctrine changed, dated, newest first.

`SETTLED.md` records what was removed from the *product*. This records what changed about the
*rules*, so a future reader can tell a deliberate reversal from an accident. §16's protocol requires
an entry whenever a rule is added, changed or retired.

---

## 2026-08-24 — §8: a modal's one commit may fill with the accent

The rebuilt swap picker (`SETTLED.md`, 2026-08-23) puts a `Today` / `Every week` `SegmentPill` pair
in every candidate row, and a selected pill is an accent border over an accent@0.15 wash. Against a
list of those, the level-① light capsule at the foot of the sheet stopped reading as the commit and
started reading as one more chip — the brightest thing on screen was whichever row you had armed, not
the button that would act on it. Antho's call, and it is the right one: the capsule that ENDS a
selection has to out-rank the selection.

**§8 — `ForgePrimaryCapsule(accent = true)`.** One parameter on the existing primitive, not a second
component. `ForgeHeroAction` already owns accent-filled at full weight, and its own note records why
a lookalike drawn from a second definition is the failure mode here: Home's orange hero beside
Cardio's white capsule was exactly that complaint. So the accent option lives on level ① rather than
relaxing the hero's hub-tab-only rule.

Scope is deliberately narrow, and the wording in §8 carries it: **a MODAL's one commit**. A page's
do-it-now action stays light, a hub tab's stays `ForgeHeroAction`. The contrast is safe on every
preset for free — the label rides `onPrimary`, which flips to the background tone above luminance
0.18 (§14), so the default Red gets dark text at 4.53:1 and the dim presets keep near-white.

Also settled the same turn: the picker OPENS with the lead candidate's `Today` armed rather than with
nothing selected. The library is ordered best-first, so the likeliest swap is one tap from done, and
the confirm is on screen from the moment the sheet opens instead of being a control that materialises
once you have guessed that a row is tappable. Selection is radio-style — re-tapping the lit pill is a
no-op, because a sheet whose confirm can vanish is worse than one that always has an answer ready.

---

## 2026-08-22 — Onboarding rebuilt: the plan is visible while you answer for it

First run was fifteen screens that showed the user nothing the app had made until the fifteenth. §3
already said "one decision per step", and the flow obeyed it exactly, which is how it ended up with
five screens of *settings* (name, units, body, watch, app lock) standing in front of the plan, and a
progress rail whose denominator had to guess the path length because the plan-mode fork sat at
question six. Three rules changed.

**§3 — an onboarding path asks only what the thing being set up needs.** The name, the units, the
body numbers, the watch brand, the app lock, the plate weight, the sore spots and the refresh cadence
are all settings: each has a real home in Settings, each has a defensible default, and none of them
shapes the plan. They moved to one optional closing step that lands AFTER the week exists, where the
honest answer to all of it is to walk past. What stayed in the path is what the generator cannot
work without: mode, goal, experience, days, gym, gear. Fifteen steps became eight, and four on the
custom / freestyle paths.

**§3 — the thing being built is drawn under the question, outside the page slider.** Onboarding used
to hold its result to the end, so every answer landed in the dark and the preview was a reveal. The
week now sits below every plan-shaping question as a bar per training day, and because it is outside
the `AnimatedContent` it holds still and animates its own values while the questions slide past it.
This is Product Principle 2 ("show the reading, not just the verdict") applied to setup: picking four
days draws the split, picking a sparse gym visibly costs you sets.

The constraint that makes it honest, and the reason it is a *rule* and not just a nice screen: the
mark must be real at every stage it is shown, or absent. It draws from the day-count on because the
split's planned volume is computable from a day-count alone (`ProgramGenerator.plannedSetsPerDay`),
and it does not exist on the three steps before that, because on those steps nothing about the week
is known yet. A row of grey placeholders standing in for a plan would be the ghost-data ban (§12)
with extra steps.

**Measured, then changed: the mark's variable.** The first build metered "exercises your gear
supports, out of the slots the split wants". Rendered, it barely moved — the generator keeps a
last-resort bodyweight fill, so even a bodyweight-only setup lands 24 of 25, and four full bars said
nothing. Sets per day varies with the split, the experience level AND the equipment, so that is what
the bars carry. A meter whose needle never moves is decoration; this is why §0 step 4 is "look at
it", not "reason about it".

**Second pass, same day — three things the first build got wrong, all of them found by looking at
the render rather than by reasoning about it.**

*The closing step read as a wall.* Eight full-width blocks of identical capsules, no rank in any of
them, and the two unit segments stacked into what looked like one 2x2 grid of four options with two
lit. It reads down a label-left spine now wherever a control fits beside its name, and spends the
full width only on the things that need it. **The rule that came out of it: a settings-shaped page
inside a question-shaped flow has to change register**, or it reads as more questions crammed
together.

*The sore-spot question was a chip cloud at the bottom of that wall.* It decides whether an injured
lifter is handed the movement that hurts, and it was four words below the plate-weight picker. It is
its own page now, head-to-toe in two groups, each spot carrying how many of the movements THAT gym
supports load that joint — and it sits BEFORE the week, so the week the user approves is already the
one their flags produced. Asking after the preview shaped a plan they had already signed off on.

*The week meter ran off the gutter at seven days.* The first build gave each bar a minimum column
width and scrolled the row when they no longer fit, which on any phone narrower than the dev device
clipped the seventh day at 100% font scale. Bars share the width evenly now and the day name clamps
at two lines, which §14 already allows a mono label to do. **A mark that scrolls sideways inside a
page reads as broken, not as scrollable.**

**Third pass — the icon family's weight was the regression, not its shapes.** `OnboardingIcons`
opens by saying it is "the SAME matched family and single visual weight as `NavIcons` and
`SettingsIcons`". Those two draw every line at 1.7–1.8; the rebuild set onboarding's to 2.2 and made
every tile in the flow visibly heavier than the same glyph weight everywhere else in the app. That
is the whole reason the redrawn grid read worse than the mismatched one it replaced — it was more
internally consistent and less consistent with the app.

The rule that survives: **one weight for lines, and it is the app's weight, not this file's.** Fills
stay free to be masses (a plate, a pad, a weight stack, a flexed arm) — what is banned is a line
drawn at one weight here and another there. Checking the constant against the two families it claims
to match takes one grep, and would have caught this before any of the shapes were redrawn.

Six glyphs also came out unreadable and were redrawn against a magnified sheet rather than reasoned
about. Two of them needed a fourth pass, and the reason is the useful part: they were judged on a
magnified sheet and **only checked at 26dp after the fact**, which is the size a tile actually
renders. A figure hanging from a pull-up bar and a stroked pulley on a cable both read at 84dp and
collapsed to a bare T and a bracket-with-a-dot at tile size. **Review a glyph at the size it ships
at; the magnified sheet is for diagnosing a shape, not for approving one.**

The other pattern worth keeping: **every attempt to draw a piece of gear as the bare object came
out as furniture.** A bar on posts is a table; hooks over a bar are humps; straps
meeting at a point are the letter A; a circle on a stick is a map pin. The ones that finally read
either show the thing in use (bodyweight is a figure mid-movement), give the object a second element
no piece of furniture has (the rack's feet and racked bar against the Smith's closed frame), or —
where this grid already held four posts-and-a-crossbar objects — take a silhouette nothing else in
the set can be mistaken for (the pull-up bar is a wide shallow U; the cable machine hangs its bar
off ONE end of the arm, because centred it is an I-beam whatever the parts weigh). A flexed arm, likewise, only read once the bicep
rose clear of the arm instead of sitting on it.

**§3 — the progress rail is segmented, one cell per step of the path actually taken.** A continuous
bar has to know its denominator, and before the fork this flow does not: the old code assumed the
longest path so the fraction would only ever jump forward. Cells sidestep the problem by counting
instead of proportioning, and committing to the short custom / freestyle path visibly drops the four
cells that will never run.

**The chapter eyebrow is gone.** "ABOUT YOU" over "About your body" was the rail's job said twice in
a quieter voice. Every other §3 archetype keeps its eyebrow; onboarding is the one place where a
second position-marker sits three lines above the first.

**A dialog is for a decision that must interrupt.** The coach opt-in was an `AlertDialog` fired at
the end of the custom / freestyle paths. It is a boolean with a sane default, so it is a row on the
closing step now, offered on every path instead of two — §12 already said dialogs are for
destructive or irreversible acts, and this was neither.

---

## 2026-08-20 — Settings rebuilt against §3: one row rhythm, gutterless capsules, themed containers

An audit of `ui/settings/` against the doctrine found the archetype was being followed in spirit and
missed in almost every mechanical particular. Four things changed about the RULES; the rest was the
code being brought to rules that already existed.

**§5 — the container family is themed now, not paid for per call site.** `ForgeTheme` never set
`surfaceContainer*`, `outlineVariant` or `surfaceTint`, so every `AlertDialog`, `DropdownMenu`,
`ModalBottomSheet` and `DatePickerDialog` fell through to Material's stock dark palette — a lighter,
purple-leaning grey belonging to no theme here. §5 stated this as a fact of life and told every call
site to pass `containerColor` itself. Eleven settings call sites never did, which is the predictable
outcome of a rule that needs remembering at N sites instead of being true once. `pearlColorScheme`
now carries the whole ladder (bg → surface → surfaceVariant, warm) and points `surfaceTint` at
`surface` so M3's tonal elevation cannot accent-wash a sheet. The call-site rule stays, because
saying `containerColor = surface` on a modal still documents intent — it just is no longer load-bearing.

**§8 — the settings capsules are gutterless, and `SettingsActionRow` owns the gutter.**
`SettingsPrimaryAction` / `SettingsOutlineAction` baked `fillMaxWidth().padding(horizontal = 24)`
into themselves. That made them correct in exactly one position — bare at page level, which only the
recipe used — and wrong in all nine shipped call sites: four double-guttered them to 48dp inside
another padded Row, two put them in a `ChipFlow` where each `fillMaxWidth` capsule claimed a whole
line and rendered the Generate cluster as the stacked button wall `FAILURES.md` names, and one sat
two of them in a plain Row where the first took the full width and the second got the remainder.
A primitive that every caller uses wrongly is a primitive with the wrong shape. The capsules now size
to their label and the gutter, spacing and large-scale WRAPPING live in one `SettingsActionRow`.

**§6 — a sentence is never mono.** Every control explainer in Settings was `labelSmall`: 10sp MONO,
the app's smallest size, in the voice §6 reserves for UPPERCASE micro-labels and explicitly bars from
sentences. Rendered at 200% they were also the first thing to wrap to three lines. They are
`bodySmall` via `SettingsExplainer` now. The recipe golden shows the payoff plainly — the same
sentence that wrapped to two lines in mono fits one in sans.

**§7 — settings rows share `SETTINGS_ROW_PAD`.** §7 already demanded one vertical padding per lens;
settings used ten (2/4/5/6/8/9/10/11/12/14), so Recovery's signal rows sat 6dp taller than the
write-back toggles directly under them. Named constant, like coach's `COACH_ROW_PAD`.

**The one that was purely a bug.** `ForgeSwitch` draws a 40×24dp track and `ToggleRow` applied no
click of its own, so every toggle in Settings — about twenty — was a 24dp-tall target against §14's
48dp minimum. The row is the target now and the switch is drawn (`onCheckedChange = null`), which
satisfies §2③'s no-nested-taps rule in the same move. `GoalEditorScreen` still calls `ForgeSwitch`
as its own target and was left alone: it is outside this pass, and the same fix applies.

**What the gate said.** 62 violations paid down across seven rules, none added; baseline 933 → 871.
`ui/settings/` now carries zero debt on `divider`, `em-dash`, `font-size`, `max-lines`,
`screen-name-title` and `unlabelled-clickable`. Both settings recipe goldens were re-recorded after
looking at the diffs (§14: a changed golden is a question) — the 200% one confirms the action row
wraps rather than overflowing, which is what the baked-in gutter had been trying and failing to fix.
## 2026-08-20 — §1/§3: Coach becomes an account, and the ledger spine is a sanctioned data line

Antho, opening the task: "I hate everything in the coach app." The page was not out of compliance —
it was one of the most doctrine-annotated files in the repo, nearly every block carrying a comment
justifying itself against a clause. That is the finding, not a defence. **A screen can satisfy every
rule in this document and still have nothing to say**, because the doctrine governs how ink is
spent, never what the screen is about. Nobody had decided what Coach was about, so it defaulted to
rendering the engine: eight sections across three lenses, the largest thing on it a count of pending
admin ("2 proposals", 52sp), and its most valuable content — a proposed change with its reasoning —
set as 14sp body text beside a 110dp thumbnail.

**The rule that changed.** §3's Overview archetype gains an **account variant**, and §1's
lines-are-data clause gains the ledger spine by name.

**Why an account rather than a better dashboard.** The two real usage scenes (confirmed with Antho)
are the Monday ritual and idle browsing, and the three lenses served neither: the Monday decision
and its evidence were one tap apart, and "did that change work" was a third tap away in Journey.
Ordering by TIME instead of by topic collapses all three into one column, and it removes the
duplication policing the lenses required — §4.3's "one home" was being enforced by hand across
three views of the same facts, which is why so much content had been cut for merely echoing
something on another lens.

**Why one filled tile is not a §1 violation.** §1 earns surfaces by interactivity, and the open call
is the only interactive entry on the page. Spending the page's single fill there makes rank
*visible* rather than typographic: the one thing asking for a decision is the one thing with a body,
and it loses that body when it resolves. This is the clause working as intended, not an exception
to it.

**Why the spine is data.** §1 bans hairlines because a line is a claim about data. The spine makes
that claim truthfully: it is the time axis every entry hangs from, which is what lets an open
proposal and a five-week-old outcome be the same object, differing only by node and stamp. It is
drawn inside the gutter at x=10dp so all four regions keep the one 24dp content column.

**What the gate said.** The change paid down four allowlist entries (two files' worth of `fontSize=`
and `maxLines=1` debt) and introduced none: the phase label wraps instead of truncating now that it
takes `labelSmall` from the scale, and `humanizeMachineProse` gives the em-dash/paren-plural
translation exactly one home. 944 tests green, screenshot goldens included.

**Not promoted to a seventh archetype.** A ledger with a spine, nodes and stamps is one screen's
pattern until it is three. Minting an archetype would owe `LedgerRecipe.kt` and a new golden set for
a vocabulary nothing else uses yet. If a second surface wants it, promote it then (§2⑥).

---

## 2026-07-27 — §4.6: the chrome slot goes to a notifications bell, and banners stop being a pattern

Home opened with up to four stacked strips (milestone, coach brief, orphan notice, resume reminder)
and Cardio with a fifth. Each was individually defensible and collectively they meant the two most
important pages in the app could push their own answer below the fold — the exact thing §1 spends
its rules preventing. They also each invented a dismissal rule, so "how do I make this go away"
had five answers.

**The rule.** A notice is not page content. Anything dismissible, celebratory or "waiting on you"
now lives in one feed (`ui/notifications`), and a page never opens with a strip above its answer.
The doctrine gains that clause in §4.6 rather than a new section, because it is a chrome rule.

**Why the bell took the wordmark's slot.** The feed needs an entry point on every screen, and the
top-left chrome slot was held by `• Avex` — a brand mark that cost a tap target on ~20 screens to
say something the user already knows. Antho called it: the wordmark goes, the bell takes the slot.
The wordmark's go-Home shortcut survives as the bell's long-press, so nothing was actually lost.
"Avex" is now the cold-launch beat (`AvexIntro`) and the signature on exported artifacts only —
those are moments where naming the app is the point, which chrome never was.

**What the gate said.** The change PAID DOWN 11 doctrine violations across five buckets with no new
ones: the banners carried the off-ladder alphas (0.10 / 0.12 / 0.14), seven `fontSize=` call sites,
the one unlabelled `.clickable(`, and the "Volume beast" hype string §11 bans by name. That is the
ratchet working as intended — the debt was concentrated in exactly the surfaces being deleted.

**One thing deliberately not built: per-row dismissal.** An `×` inside a row is a tap nested in the
row's own tap target (§2③). Acting on a row clears it, and `Clear all` sweeps the rest with an Undo
(§12). If a per-row clear is ever wanted, it has to be a swipe, not a glyph.

**Where Clear all lives, revised twice the same day (Antho).** It started as an outlined capsule at
the end of the scroll, per §8's "group page-level actions at the END". That rule exists so buttons
don't interrupt a *scroll* — but this page's scroll is a handful of rows, so the capsule was the
last thing on an otherwise empty page, next to nothing it related to. It became a top-bar dropdown,
then a **bottom sheet** on Antho's call: a dropdown pinned to the corner is a menu, a sheet is a
place, and the two actions here are page-scoped decisions rather than a corner afterthought. §8's
end-of-page rule still holds for settings and long pages; a short list whose only actions are
page-scoped is the case it doesn't cover.

**Rows lost their border, and that changed what the arrow is for.** §1 earns a surface with
interactivity, which had every actionable notice in its own outlined box — and stacked, those read
as a wall on a page whose whole job is to be scanned. The border is gone and the leading glyph sits
in a chip instead. That inverts §1 on its face, so the reading that makes it hold: **the chip is the
row's MARK (§12), and boxing a mark is not boxing passive content.** The consequence is that the
accent ` →` is now the ONLY thing distinguishing a row that goes somewhere from one that just
happened — it stopped being a flourish and became the affordance, which is why passive rows must
never get one. The mono eyebrow went at the same time: the glyph names the kind, and §4.3 gives a
fact one home. The glyph therefore carries the kind in its `contentDescription` (§14) — dropping a
visible label only works if something still says it aloud.

**`error` as a label, refused.** The reference had "Clear all notifications" in red. §14 measures
`error` text at 3.69:1, fails it, and forbids NEW error-coloured body text until that's resolved —
naming this exact substitution as the way through. So the bin glyph is tinted and the label stays
onBg. The doctrine gets to win over a reference screenshot when it has a measurement behind it.

**The settings page split by DESTINATION, not by feature (Antho).** Adding the in-app toggles left
Settings → Notifications saying two different kinds of thing under one word, with "Quiet hours"
trailing the in-app switches — where it read as though it might silence those too. It can't; there
is nothing to silence, an in-app row makes no sound. The page is now two groups named by where the
thing ARRIVES — `ON YOUR PHONE` (quiet hours folded in as one of its rows, since it suppresses
exactly those three) and `IN THE APP`.

**The first attempt captioned both groups, and Antho called it cringy.** "Push alerts, delivered
even when Avex is closed" / "Waiting under the bell. Never sent to your phone." Both were explaining
push notifications to someone who has owned a phone for fifteen years. The fix was not a better
sentence — §4.3 says mechanics narration is CUT, not trimmed — it was making the headers a parallel
pair (`ON YOUR PHONE` / `IN THE APP`) so the split needs no gloss at all. Two labels in the same
grammatical shape do the work a paragraph was doing badly.

The same test then caught the row explainers: "Unfinished workouts · *A workout you started but
haven't finished yet*" is a definition of its own label, not information. Every `NoticeKind`
explainer now says what the ROW will offer ("Resume where you stopped"), which is the only thing a
reader doesn't already know. **The general rule: an explainer that could be derived from the label
is condescension with extra steps.**

**And the bug underneath it.** `NotificationsPage` never had `.verticalScroll()`, unlike every
sibling settings page. It fit before, so nobody noticed; five new toggles pushed quiet hours off the
bottom with no way to reach it. Worth remembering that a missing scroll modifier is invisible until
content grows — the page does not look broken, it looks short.

**A test, prompted by a false alarm.** Undo looked broken on device: cleared the list, tapped Undo,
nothing came back. The cause was `SnackbarDuration.Short` (4s) expiring during the gap between two
tool calls, not the code. Rather than re-tap and hope, `NotificationPrefsTest` now round-trips every
write the undo lambdas make. It also pins the fix that fell out of the same session: both setup
invites are dismissed for good with no un-dismiss control, so `SettingsSection.NOTIFICATIONS` had to
grow to cover them — "reset this section" now genuinely restores every switch on the page.

**Per-notification toggles, and why they're a filter rather than a mute.** Each `NoticeKind` is one
switch on Settings → Notifications. Stored as the DISABLED set so kinds added later default ON, and
applied as the LAST step of the feed — the notice is still QUEUED while its kind is off. Switching
it back on brings the row back rather than revealing that it was silently dropped, which is the
difference between a filter and a mute, and the honest one of the two.

**A fourth icon family.** `NoticeIcons` joins `NavIcons`/`SettingsIcons`/`ExerciseIcons` rather than
reaching for Material stock (§8 permits stock in TOP-BAR chrome only). The coach brief reuses
`NavIcons.Coach` outright instead of drawing a second compass — a glyph, like a fact, gets one home.

**Then the same test was applied to the modals.** Three `MainActivity` dialogs failed it: the
share-import result and the backup-restored confirmation were both "here's what happened · OK" over
whatever screen you were on, and the POST_NOTIFICATIONS rationale interrupted a cold launch to ask.
All three are rows now. The permission one opens the OS app-notification screen instead of
re-requesting — that keeps working after any number of denials, which a re-request does not.

The line that decides it: **a dialog is for a decision the app cannot proceed without; a notice is
everything else.** By that test `ProgramChangeGuardHost` stays (discarding an in-progress workout is
destructive and irreversible), and `CheckinSheet` and `DislikeSwapPromptDialog` stay (they ask for
INPUT at the moment it is relevant, which is not the same as asking for attention). Settings →
Notifications keeps its blocked banner too: it is the denied-state of the controls directly beneath
it (§12), not a notice about something elsewhere.

## 2026-07-27 — The Academy gets its tracks, and locked lessons start saying something

Antho: "it needs to be like a real academy UI and the tasks are not good at all, nothing is
explained, it's just useless." Both halves were fair, and the fix for the first one was already
written down — I was two edits into building the wrong thing when he told me to go read
`docs/ACADEMY_LESSONS.md` and `COACH_V3_PLAN.md` first. **Read the plan before redesigning the
thing the plan describes.**

**What the plan already said.** Lessons are "grouped into five tracks" — and the shipped screen
grouped by nothing, rendering 31 rows as one flat UNLOCKED/AHEAD list. So the "real academy" ask
and the plan agreed; the build had just skipped it. Tracks now carry the structure, each with a
dot rail and its own page.

**What the plan forbade, which I was about to build.** "Just-in-time, not curriculum-first —
lessons attach to coach moments, not a course index", and "no XP; learning is not gamified
engagement bait". A course-progress UI with percentages and a next-up ladder would have violated
both. Hence a filled/hollow **rail** rather than a progress bar (inventory, not score), and a
`START HERE` that offers only a lesson whose moment has ALREADY fired — never the next item of a
syllabus.

**The tasks.** `unlockedBy` was one string naming an internal moment: "Your first placement-driven
prescription", "The first time a personal volume cap changes an allocation". Accurate, and useless
— half of them were the vocabulary the lesson itself exists to teach (§11, translate the machine).
It is now `LessonUnlock(label, detail, byYou)`, and the split is the whole point: the plan's own
trigger taxonomy is app-usage moments vs coach-ledger moments, so `byYou` says whether the next
move is yours. Yours reads as an instruction with an accent dot ("• Log a set"); the coach's names
the moment ("When a block changes phase") and never pretends to be a task you could go and do.
`AcademyRegistryTest` enforces that grammar in both directions, and that a detail line never just
restates its label.

## 2026-07-27 — The bell goes Home-only, and the screenshot gate is weaker than advertised

**Home only.** The bell sat in all ~20 top bars. An unread badge that follows you into every screen
you navigated away from it to reach is a nag, not chrome — and it made the one piece of chrome that
ISN'T about the current screen the most persistent thing in the app. §4.6 now reads `←` + ≤1 action,
with the bell named as a Home exception. It also settles the alignment split noted below: with the
bell gone from `TopAppBar` screens there is no second inset left to disagree with Home's gutter.

**The Profile glyph was redrawn for chrome.** `NavIcons.Profile` was built for the tab bar — wide,
bottom-heavy, shoulders chopped flat by the viewport edge. That reads fine above a text label, where
a glyph is a silhouette; beside a bell it read as squat and cropped. It is narrower now (12.2 units,
near the bell's own width), lifts off the baseline and rounds its base corners. **A glyph is drawn
for a slot, not for an app** — moving one between slots is a redraw, not a re-reference.

**And a caveat worth writing down: `verifyRoborazziDebug` did NOT catch this change.** Removing the
bell from all six recipe top bars left the goldens stale, and verification passed — twice, including
under `--rerun-tasks` and with `-Droborazzi.test.verify=true` set by hand. Planting a knowingly-wrong
golden also passed. The cause is `changeThreshold = 0.001f` in `RecipeScreenshotTest`: one 20dp glyph
is ~0.1% of a 1078×2399 frame, right at the edge of the tolerance. So the gate catches LAYOUT shifts
(the wordmark→bell swap moved every row down and failed loudly) but can miss a single small element
appearing or vanishing. Treat "goldens green" as evidence about layout, not about content, and
re-record deliberately when a change is small and local.

Three moves, all Antho's call, all in the same direction: **put the thing where you'd look for it.**

**Academy takes Profile's tab.** The Academy is the half of the coach you can read — it is why the
coach can be optional at all — and it was a text link inside the Coach page, one tap deeper than the
thing it explains. It is a tab now, and the Coach link is gone (a link to a sibling tab is redundant
navigation, §4.2). The `newLessons` count that link carried has no home yet; if it's wanted, it
belongs as a badge on the tab, not as a second entry point.

**Profile takes the Settings slot on Home, and Settings moves inside Profile.** Settings was the one
piece of chrome on Home that wasn't about you or your training. Profile is; and Settings is
reachable from it, which is where you'd already go to change something about yourself. Profile stops
being a hub page and becomes a pushed route with its own back arrow — one fewer thing you can swipe
into by accident.

**Chrome glyphs meet the page gutter.** Home's bell was centred in its 44dp touch target, which put
the glyph 12dp inboard of the 24dp gutter — so it lined up with nothing, least of all the serif hero
directly under it. `GUTTER_SLACK` (12dp: half the gap between a 44dp target and a 20dp glyph) pulls
both glyphs out so their EDGES sit on the gutter while the targets keep their full size. The lesson
generalises: a touch target is padding, and padding should never decide where a glyph appears to be.
`TopAppBar` screens still use Material's own title inset (~4dp wider); not worth 18 files to chase.

## 2026-07-25 — §6 gains a mono anchor rung: equal size is not equal presence

Row labels moved from 11 → 13 to stop being the smallest thing on a page whose job is naming metrics
— which put them on the section anchor's own rung. A section anchor is usually a SHORT word with wide
tracking ("BODY") sitting over long rows ("BODY FAT"): at a matched 13sp both measured an identical
26px cap height on device, the anchor carried less visual mass, and the header read as the *smaller*
of the two.

So `EditorialHeader` now takes `MonoSectionAnchor` (15sp) — one step above the rows beneath it. The
general rule §6 keeps: **when two labels must rank, rank them by SIZE**; tracking and colour cannot
carry a hierarchy on their own.

## 2026-07-24 — Fourth audit pass: reviewing the templates as design, not as regex

Much quieter round: one real defect, one hardening, and three angles that came back clean.

**1. `LiveRecipe`'s stepper carried dead code.** A `Spacer(Modifier.height(0.dp))` inside a *Row* —
where a height does nothing at all — while the actual spacing came from a leading space inside the
string (`" $unit"`). Both wrong, and worse in a file whose entire job is to be copied: a template
teaching a no-op and a spacing hack. Replaced with a real `Spacer(Modifier.width(4.dp))`.

**2. Sub-references were unguarded.** `§4.10` resolves only while §4 has ten numbered items; reorder
that list and every sub-reference silently points at nothing, which the parent-section check cannot
see because §4 itself still exists. Added `everySubReferenceResolves`, proved by referencing §4.99.

**Came back clean.** The comment-stripper survives eight edge cases (a `//` inside a string, an
escaped quote before one, URLs, raw-string bodies, and violations hidden in comments — which are
correctly ignored rather than flagged). All existing sub-references resolve. The parity tests fail
loudly, not silently, when the doc format is mangled.

**Left as a judgment call, not changed:** in the Live stepper the `−` and `+` sit at the screen
edges with the figure left-aligned between them. Defensible, and common in other apps, but §3's Live
archetype prioritises reach over padding and the `+` is a long way from a thumb. Flagged rather than
redesigned, because that is a taste call and taste calls are Antho's.

---

## 2026-07-24 — Third audit pass: the gates were agreeing with themselves

**1. A line break disabled three rules.** The scanner matched line by line, so `title = {` with
`Text(` wrapped onto the next line read as compliant, as did a split `maxLines = 1` and a wrapped
`tween(`. No such splits exist today, but a redesign that reformats a long call would introduce them
silently. Those three rules now match against the whole comment-stripped file with a line-number map;
counts were unchanged, which is the evidence that no false positives came with it. Verified by
injecting a wrapped `title = { Text("Settings") }` and watching it get caught at the right line.

**2. The screenshot goldens could not see a truncation bug.** Adding `maxLines = 1` plus an ellipsis
to a list row — the exact §14 violation — changed **zero** goldens, because every fixture rendered
short strings like "Pull B". The visual gate was agreeing with itself. This was the doctrine's own
rule broken in its own test fixtures: §12 says design against the longest realistic string, not the
demo one. Added long-content goldens (realistic exercise names at 100% and 200%); the same bug now
fails `list-long-200`. 16 goldens -> 20.

**3. The §9 duration check passed on a coincidence.** It looked for each duration's number anywhere
in the section, and deleting "Draw 900" from the table still passed because an unrelated 900 (the
ForgeSwitch spring) sat a few lines below. Now matches the labelled pair.

**Probed and sound:** the other parity tests fail loudly when the doc format is mangled — renaming a
§6 voice marker, rewording the §7 radii line and deleting a §14 table row each fired the right test
rather than silently matching nothing.

---

## 2026-07-24 — Second audit pass: false negatives, and doctrine filed as debt

**1. `.clickable { }` was invisible to the accessibility rule.** The regex required a parenthesis,
but Kotlin's trailing-lambda form has none — **24 unlabelled tap targets** never reached the gate.
Widened to `\.clickable\s*[({]`; debt rose 906 -> 930, all of it previously hidden.

**2. Thirteen violations were the doctrine's own implementation.** `ripple` in `BounceClick.kt` is
§9's TalkBack fallback; `SnackbarHostState` in `SnackbarControllerHost.kt` is §8's one Undo snackbar;
the literal palette in `ConfettiOverlay.kt` is the one deliberately polychrome moment. Counting the
shared implementation of a rule as a violation of it made the debt figure lie in the *other*
direction, and no amount of cleanup could ever retire those entries. Moved into a `SANCTIONED` map in
the scanner, with a test that fails if one of those paths goes stale — otherwise a rename would
silently un-exempt the file and the fix would look like "add it back to the allowlist".

**3. Robolectric fetches its `android-all` jars at test time into `~/.m2`,** outside Gradle's
dependency resolution, so `setup-gradle`'s cache never covered them. Every CI run would have
re-downloaded a few hundred megabytes and inherited Maven Central's bad days as flaky tests. Cached
explicitly in `ci.yml`.

**Checked and clean:** raw strings hold only shaders, SQL and the MIT licence, so the scanner's
inability to see inside multi-line raw strings costs nothing today; `Card (` with a space does not
occur; `AppIconManager` only ever toggles its own `AppIcon.entries`, so the new debug launcher icon
cannot collide with it; PNG goldens need no `.gitattributes`; and the screenshot gate genuinely
verifies when run after the unit-test step rather than being skipped as up-to-date — checked by
breaking a golden and watching five fail in CI order.

---

## 2026-07-24 — Audit pass: four more defects before trusting any of it

Deliberately hunting rather than re-running. Everything below was wrong and is now fixed.

**1. Four packages of user-facing copy were never scanned.** The copy scope covered `ui`, `domain`,
`service` and `data`. It missed `program`, which holds every exercise description rendered in the
swap picker — **93 em dashes** sitting there unseen — plus `widget` (home-screen copy), `security`
and `appicon`. Added all four. Frozen debt went 810 -> 906, which is the net getting wider, not the
code getting worse.

**2. `alpha = 0f` and `alpha = 1f` were false positives.** The regex strips a trailing `f`, so it
captured "0" and "1", neither of which was on the ladder. Fully transparent and fully opaque are not
ladder choices; both are now allowed.

**3. The recipe gallery was unreachable.** It previewed in the IDE but nothing referenced it, and the
nav host lives in `main` and must not know debug code exists — so "browse the archetypes on a real
device" was simply false. Now a debug-only `RecipeGalleryActivity` with its own launcher icon,
verified present in the debug manifest and absent from release.

**4. Goldens had zero pixel tolerance.** Font rasterisation differs between machines and JDKs, so
goldens recorded on a dev box tend to fail the first time CI renders them for reasons unrelated to
design. Added a 0.1% threshold, then checked it is not too loose: a 4dp spacer change still fails
five goldens.

**Also verified this pass:** a genuinely cold build (build directories deleted, build cache and
configuration cache disabled) runs 668 tests green and reproduces every golden; `:wear` still builds
with the Roborazzi plugin applied to `:app`; nothing new is caught by `.gitignore`; multi-line raw
strings (101 of them, mostly AGSL shaders) do not produce false positives.

---

## 2026-07-24 — Redesign-readiness: per-token allowlist keys, and a safe paydown mode

**Why.** Pressure-testing the gate against the workflow a UI revamp actually puts it through, rather
than just re-running it.

**A third allowlist design, because the second still had a hole.** Per-FILE counts meant a fix and a
new violation in the same file cancelled out: cleaning up three alphas in `OverviewTiles.kt` while
introducing a fourth netted to a decrease, so the new one was invisible to the gate *and* to the
paydown check. Keys are now **(rule, file, token)** with exact counts — a new value is a new key, and
more of an existing value raises its count. 386 entries, same 810 total. Verified by reproducing the
exact case that slipped through; it now fails both checks.

**`-Dforge.paydown=true`.** Rewriting screens removes old violations, which the gate reports as
"debt was paid down, lower these numbers". Correct, but a slog across thirty files, and the tempting
escape was a full `-Dforge.regen=true` that would silently swallow any new violation from the same
pass. Paydown lowers counts and **refuses to raise any**, failing with the offender named and
writing nothing. That keeps the ratchet honest while a redesign is in flight.

**Gotcha worth knowing:** a system property alone does not invalidate a Gradle test task, so the
maintenance commands need `--rerun-tasks` or they no-op and report success. Documented in `SKILL.md`.

**And the worst one, found last.** The doctrine tests read `.claude/` from disk, which Gradle cannot
infer from the classpath, so the test task stayed UP-TO-DATE whenever only documentation changed —
meaning `DoctrineParityTest` and `DoctrineSelfCheckTest`, the two suites whose entire job is catching
doc drift, **did not run when the doc drifted.** A stale 99-line `SKILL.md` sat past its 80-line cap
through several green builds. Fixed by declaring `.claude/` as a task input; verified by editing only
`DESIGN.md` and watching the suite fire. The loader cap was then raised 80 → 110 deliberately, since
what pushed it over was the redesign workflow (process, which belongs there) rather than restated
rules (which do not).

---

## 2026-07-24 — Screenshot goldens (Roborazzi), and the first bug they caught

**Why.** Everything else in the repo checks source text. Clipping, overlap and spacing regressions
live in rendered pixels, and §14's "must survive 200%" was a promise nothing could verify.

**It worked, against expectations.** Paparazzi's stable line (1.3.5) couples tightly to AGP
internals and this project runs AGP 9.2.1 / Gradle 9.4.1 / Kotlin 2.2.10, so the spike used
**Roborazzi 1.70.0 + Robolectric 4.14.1**, which sit on the unit-test side. It resolved and ran
first try. Full record takes about two minutes cold, ten seconds warm.

**16 goldens**, committed under `app/src/test/screenshots/`: the six archetype recipes at 100% and
200% font scale, plus Overview at zero data, on AMOLED, and in monochrome. CI runs
`:app:verifyRoborazziDebug` and uploads diffs on failure. Verified it actually detects a regression
by changing one `Spacer` from 2dp to 18dp — four goldens failed.

**What it found immediately.** `SettingsPrimaryAction` and `SettingsOutlineAction` had no horizontal
gutter and no width bound, so the capsule sized to its label's intrinsic width. Fine at 100%; at 200%
"Update Health Connect" ran off **both** edges of the screen. Shipping code, invisible to every
static rule here, and invisible to the eye at normal font scale. Fixed by bounding each capsule in a
`Row(fillMaxWidth + 24dp gutter)`.

It also made the accent-contrast problem concrete: `Manage permissions →` at 200% is visibly dim
against the ground, which is the 2.35:1 measured in `SETTLED.md` rather than a rendering artifact.

**Re-record deliberately.** `:app:recordRoborazziDebug` overwrites the goldens. A changed golden is
a question, not a chore: look at the diff before accepting it.

---

## 2026-07-24 — Enforcement round 2: widened scan, 9 new rules, doc↔code parity

**Why.** The gate written earlier that day had a hole in exactly the place it mattered most: §11's
copy rules are written for *generated* lines (coach, milestones, recaps, notifications), but the scan
only covered `ui/`, and generated copy lives in `domain/` and `service/`. Five violations were
shipping, two of them in notifications.

**Changed.**

- **Scan roots are now per-scope.** `ui/` takes every rule; `domain/`, `service/` and `data/` take
  copy rules only; `:wear` takes copy + colour. Layout rules deliberately do not run outside `ui/` —
  a gate that cries wolf gets ignored, and clamped text is normal on a round watch face.
- **`:wear` is gated at all for the first time.** It was hardcoding the phone palette with nothing
  connecting the two, while `WEAR.md` said they "may not drift".
- **Nine new rules.** `hype` and `paren-plural` (§11); `m3-card` (§1), `spinner` (§12), `rtl` (§14),
  `toast` (§12), `snackbar-host` (§8), `ripple` (§9), `unlabelled-clickable` (§14).
  The first three had **zero** occurrences — they cost nothing and exist purely so the doctrine's
  most load-bearing bans can never quietly regress.
- **`Icons.Filled` was considered and rejected as a rule.** §8 explicitly permits stock icons for
  top-bar chrome and forbids them in content, and a static scan cannot tell the two apart. A rule
  with 51 unavoidable false positives would have taught everyone to ignore the gate.
- **Five §11 violations rewritten** rather than allowlisted: the two notification exclamation marks,
  the "the work is showing" praise ungrounded in data, and three em dashes in coach/insight lines.
  `PrMilestoneTest` was asserting the old string and was updated with it.
- **`DoctrineParityTest`** — every value the doc states must equal the value in code: §5 colours,
  §6 type scale, §7 radii, §9 durations, §8's component inventory (both directions), §14's contrast
  table recomputed from the real palette, and wear/phone palette parity.
- **`DoctrineSelfCheckTest`** — the line cap, every `§N` reference resolving, every satellite and
  recipe existing, the loader staying a router, and the wordmark being spelled "Avex".

**Bugs this round caught in the previous round's work.**

1. *The allowlist key was too coarse.* Keys were `rule + file + token`, and generic tokens like
   "em dash in string" meant allowlisting one em dash in a file exempted every future one. Found by
   injecting a violation and watching the em-dash rule stay silent. Now **exact per-file counts**, so
   any addition *or* fix moves a number and paydown shows in the diff as "12 -> 11".
2. *The `:wear` root silently did not exist.* `File(".").absoluteFile` keeps its trailing `/.`, so
   `parentFile` returned the module rather than the Gradle root; the root was filtered out and the
   whole watch module went unscanned while every test passed. Now `canonicalFile`, plus two tests
   that assert every expected root resolves and contains sources. **A gate that scans nothing passes
   everything.**

**Line cap raised 400 → 420.** The §8 inventory is now machine-verified complete in both directions,
which added names that cannot be trimmed without making the parity test fail. Raising it deliberately,
in the same commit, is the process §16 already described; the alternative was letting the file exceed
a number it claimed to respect.

**Deferred.** Remediating the 831 frozen violations (see `AUDIT.md` for the ranked order). A
`traversalIndex`/focus-order rule was considered and left out: phone-only and portrait-only, so the
default order is almost always right, and a wrong rule here is worse than none.

---

## 2026-07-24 — The split: one doctrine file became a core plus satellites

**Why.** `DESIGN.md` had reached 299 lines / 50KB, 2.5× the ~200 lines its own loader claimed, and
was loaded in full before every UI task including one-line tweaks. 30% of it was the per-screen
inventory (§2) — the part that rots fastest and helps least when deciding *how* to build. It was also
almost pure prohibition: ~190 "never"s and zero worked examples, which can correct a design but
cannot generate one.

**Changed.**

- **Split** into a core (rules) plus `MAP.md` (inventory), `SETTLED.md` (removals), `WEAR.md`,
  and later `FAILURES.md` and `AUDIT.md`. Verified byte-identical: reconstructing the original from
  the pieces diffed clean against the pre-split file.
- **Added what prohibition cannot supply**: six decision ladders (§2), the seven-state matrix (§12),
  a physics chapter (§14 — font scale, measured contrast, touch, Canvas semantics), a one-clause
  *why* on each principle, and per-archetype checklists.
- **Six compiling archetype recipes** in `src/debug`, each showing the section rhythm, where the mark
  goes, the zero-state branch inline, and a 200% font preview. Debug-only, so they never ship, but
  they compile against the real primitives and break if one drifts.
- **`FAILURES.md`** — twelve named failure modes with symptom, cause, fix. Naming a failure is the
  point: "this is a button wall" is a diagnosis; "too many buttons" is an opinion.
- **The loader became a pure router.** It had drifted into saying `• Forge` instead of `• Avex`,
  teaching "Baseline set" as a good verdict when §11 bans that exact string, and carrying three
  off-by-one section references. It now restates nothing.

**Contrast measured, two failures found and NOT silently fixed.** Accent-coloured `action →` links
run 2.35–3.40:1 and the inline error line 3.69:1, both below AA. Both are mandated by the doctrine
itself, and fixing either changes the app's look — so they are recorded in `SETTLED.md` as open
decisions with options, and §14 forbids *new* accent- or error-coloured body text until resolved.

**A headline number was corrected.** An initial grep counted 570 fixed `.height()` calls and framed
this as a large migration. Classifying them showed 517 are `Spacer`/`Canvas`/drawn marks where a
fixed height is correct, and of 7 flagged, 5 were false positives. Real exposure: ~2 sites. The
actual 200% risk is clamped content and the type-scale bypass, not fixed heights.
