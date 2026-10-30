# Named failures

Shapes these apps have actually produced, each with how to spot it and how to fix it. Naming one
turns "something's off" into a fix: "this is a button wall" is a diagnosis, "too many buttons" is
an opinion. Most come from Avex's ledger (`.claude/design/FAILURES.md` in that repo) and the
owner's corrections in Relay and Tally. Add a new one here when you find it: name, symptom, cause,
fix.

## Layout and hierarchy

**The AI look (checklist section).** More than about four uniform rows of dot-plus-status-phrase.
Reads as machine-made at once. *Fix:* one mark plus the one detail that matters ("2 of 9" on a
segmented rail, then only the next item). Rows earn a list only when each carries its own reading
or action.

**Stat-card wall.** A grid of boxed tiles, each a label and a number, all the same weight. Nothing
is the answer. *Fix:* one hero figure, two to four supporting figures without boxes, the rest cut or
moved to a detail page.

**Figure wall.** Five or more big figures on one screen, or a dashboard figure row on a detail page.
*Fix:* one hero per screen, two to four figures in the row at most, none on a detail page.

**Caption stack.** Header, caption, hint and explainer before any data. *Fix:* one short caption
per section. A hint replaces the caption, it is never added to it.

**Strip above the answer.** A notice, tip, onboarding nudge or "connect your watch" banner pushing the
page's answer down. *Fix:* notices go behind the bell. Only a transient arrival receipt that
displaces nothing may cross a page.

**Scroll tax.** The page's useful part is below two screens of low-value sections. *Fix:* rank by
what changes a decision, cut what doesn't.

**Empty page.** A page with a title and almost nothing ("some feel empty"). *Fix:* real readings
(counts, recent items, a small chart of the page's own data). Never filler or decoration.

## Surfaces and shape

**Boxed passive content.** A grey card around something you can't interact with. Reads as dated.
*Fix:* passive content sits on the page, separated by air and a small section header (phone apps).
On Relay desktop, follow the Glass Rule's card scale: one veil per role, nothing nested repainting.

**Hairline habit.** A thin rule between every section or row. The page reads as a form. *Fix:* air
does the separating. A line exists only as data.

**Round everything.** Pills and big radii on cards, buttons, charts and backdrops at once ("you went
too far on the round UI"). *Fix:* pills for small toggles, badges and counts only; one radius
scale; adjacent elements share one shape.

**Inconsistent see-through.** One page more translucent than its neighbours ("way too see-through
compared to other parts", then "not see-through enough"). *Fix:* one translucency scale by role,
applied everywhere (Relay: the Glass Rule).

**Futuristic chrome.** Glows, neon edges, angled frames, sci-fi boxes. *Fix:* flat tonal steps,
1 px edges, the project's tokens.

## Actions

**Button wall.** Three or more filled buttons down a page, or one per row. *Fix:* one filled
primary per surface, grouped at the end. A row action is a drawn outlined pill with the whole row as
the tap.

**Chunky controls.** Apply/Skip or a popup sized for a poster ("too big"). *Fix:* standard trim
(28 px desktop keys, 44 dp phone capsules), and grow a control only together with its section.

**Fake tap / nested tap.** Something looks pressable and isn't, or a control inside a tappable row
also responds. *Fix:* nothing looks tappable while doing nothing; one target per row.

**Two back arrows.** An in-page back beside the top-bar back. *Fix:* the top-bar arrow alone.

## Data and states

**Verdict without a reading.** "Ready", "Over pace", "Deload week" with no number to check.
*Fix:* the deciding reading as row meta beside it ("38% hard · +2"). Below a gate, show progress
toward the gate ("3 of 12").

**Status-word empty.** Rows of "Not connected", "No data", "Forming". *Fix:* the section's own mark at
zero (empty track, hollow dots, honest 0).

**Empty by omission.** Each mark decided on its own to draw nothing, so a whole section is labels
and pills. *Fix:* the container (baseline, empty track, rail of slots) is always drawn; only the
value layer is conditional. Check the section's all-zero state, not just each row's.

**Invisible ghost.** An empty mark drawn at a tone that measures ~1.1:1 on the ground. It passed review
and is invisible on the phone. *Fix:* standalone tracks and rings take the empty-state rung (about
3.5:1 for thin strokes). Measure the pixels.

**Debris strip.** A row of tiny cells that, at real data volumes, is a few specks. *Fix:* design at
the emptiest realistic state; use rows at small counts.

**Two units, one section.** "4/7" above a strip with one lit cell (sessions vs days). Reads as wrong
without reading as broken. *Fix:* a mark and its caption count one unit, and the field is named for it.

**Mark echo.** The same answer twice on a screen, or the same chart on two tabs. *Fix:* one home per
fact and per mark.

**Machine leak.** "2026-W27", "3 session(s)", an enum name, a raw error, a dash for a skipped day.
*Fix:* translate at render ("Week of Jun 29", a rest-day icon, plain words with the raw error in a
tooltip).

**Grey dot column.** Identical neutral dots down a list. *Fix:* paint a dot only for the exception
(failure, live, win); other rows keep the gutter, dotless.

## Type, colour and icons

**Mute below the floor.** A caption so dim it reads as disabled. *Fix:* the project's muted floor
(measured ≥ 4.5:1). If it must recede further it is the wrong element in the wrong place.

**Accent confetti.** The accent on many small things (dots, labels, figures, borders). Reads as
dead pixels, and "stands out too much". *Fix:* few places, large size; accent never on body text.

**Silent scale break.** Text clips in a capsule or a name ellipsizes mid-word at large font sizes.
*Fix:* containers hug content, no single-line clamp on user content, sizes from the type scale.

**Icon off-family.** One glyph heavier, stock or off-grid beside a custom set; or not centred in its
key. *Fix:* draw it on the family's grid and stroke, judge it in its row at shipping size, centre
it optically.

**Unasked font swap.** A typeface changed as part of a "refresh". *Fix:* fonts change only when asked.
