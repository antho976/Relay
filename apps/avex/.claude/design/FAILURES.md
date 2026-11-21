# Forge — named failure modes

Satellite of `.claude/DESIGN.md`. Read this when **a layout feels off and you can't say why**, or when
reviewing a screen. Each entry is a shape this app has actually produced or actively guards against.

Naming a failure is the point: "this is a button wall" is a diagnosis you can act on, where "too many
buttons" is a matter of opinion. When you find a new one, add it here with the same four fields.

`§` cross-references point at the core `.claude/DESIGN.md`.

---

## Button wall

**Symptom** Three or more filled capsules stacked down a page, or one filled capsule per list row.
The page reads as a control panel; nothing looks primary because everything does.
**Cause** Treating every row's action as primary. Each addition looked reasonable on its own.
**Fix** ONE filled capsule per page, grouped at the END (§8). A row-scoped action becomes a
whole-row tap target with a *drawn* compact outlined pill on the right — the pill is not separately
clickable, so there is no nested tap.
**Seen in** Settings → Wearable, where five filled Connect capsules stacked. Now `ConnectPill`.

## Grey dot column

**Symptom** A column of identical neutral `•` dots down the left of a list. Looks structured,
carries nothing.
**Cause** Using a leading dot as a bullet rather than as a flag.
**Fix** Paint the dot only for the exception — a failure (`error`) or a win/active (accent). Common
rows reserve the gutter (`CoachFlagDot(null)`) so they still align, dotless (§8). If no row is
exceptional, there is no dot column.

## Checklist section — "the AI look"

**Symptom** A run of more than ~4 uniform dot-text rows, each a short status phrase. Instantly reads
as machine-generated.
**Cause** Rendering a list because the data *is* a list, without asking what the reader decides from it.
**Fix** ONE mark + a single focused detail (§4.10). A 9-row ladder becomes a segmented rail "2 OF 9"
plus only the NEXT item. Rows earn a list only when each carries a distinct reading or action.
**Seen in** The 9-row milestone ladder and Coach's pre-baseline signal dot-checklist, both removed
(`SETTLED.md`).

## Broken-ghost group

**Symptom** A section of nothing but flat grey lines or hollow marks. Reads as a rendering bug rather
than as "no data yet".
**Cause** Applying the ghost-mark rule uniformly instead of contextually.
**Fix** A ghost mark only reads as "still forming" *beside a live sibling* — the contrast is what
carries the meaning. An all-ghost group drops the mark entirely and collapses to ONE line naming the
concrete unlock (§12): "3 lifts building history · first read after two sessions".

## Empty by omission

**Symptom** A whole section renders with no marks at all — a column of mono labels each with an
action pill beside it, and nothing in between. Every individual rule was followed; the section still
looks like a settings page.
**Cause** Two or more marks each decided, correctly and independently, to draw *nothing* in their own
empty case. §12 says a ghost only reads as "still forming" beside a live sibling, so each mark
reasoned "no live sibling, therefore stay silent" — and with the section entirely at zero, every mark
took that branch at once. Locally sound, globally a section with no content.
**Fix** A mark's empty case is a **container, not a value**: a chart baseline, an empty meter track,
a rail of unfilled slots. Those are always safe to draw because they cannot be mistaken for a reading
of zero, which is the thing the ghost rule was protecting against. Keep the ghost-beside-live-sibling
rule for the *value* layer; put the container underneath it unconditionally. §1 already permits this
line: "a line exists only as data — chart threshold/floor/baseline".
**Seen in** Profile BODY on a brand-new account, 2026-07-24: `GhostSpark` drew nothing without a live
sibling and `MeasurementRail` returned early with nothing tracked, so WEIGHT, BODY FAT and SIZES all
rendered as label + pill. Fixed by making the sparkline baseline and the coverage rail unconditional.

**The general shape:** any rule worded "only draw X when Y" needs a defined answer for "no Y", and
that answer must not be "nothing" for every mark in the section simultaneously. When reviewing, check
the all-zero state of a section, not just of each row.

## Invisible ghost

**Symptom** A section's empty state is drawn, reviewed, and correct in code — and still looks like a
blank page on the phone. The marks are there; you cannot see them.
**Cause** Reaching for §5's **bar-track** rung (`outline @0.25–0.35`) for a mark that has nothing
filled on top of it. That rung is calibrated for a track with an accent fill riding it, where the
FILL carries the contrast and the track is only a groove. Standing alone on near-black it measures
**~1.08:1** — indistinguishable from the page.
**Fix** When the track/ring/frame **IS** the mark, it takes §12's empty-state rung, not the bar-track
one. Thin strokes (a 1.5dp ring, a 1dp frame) take `muted @0.55` — the value §8 already fixes for
`StatusDot`, measuring ~3.54:1. Solid bars take `muted @0.30` (~1.90:1), lower because a solid shape
covers far more area and must not out-shout the live reading it stands in for.
**Verify by measuring, not by looking.** A downscaled screenshot flatters low contrast; sample the
rendered pixels (`magick shot.png -format "%[pixel:p{x,y}]" info:`) and compute the ratio against the
page. Every one of these passed visual review before the numbers were taken.
**Seen in** 2026-07-25, all at once, in code written the same day *to fix* an empty-state bug:
Profile's `GhostSpark` track and `MeasurementRail` pips, the Gallery filmstrip's ghost cells, the
Gallery band's ghost frames, and Measurements' `CoverageRail`.

**The trap underneath it:** the navy accent itself measures only **2.39:1** on Pearl, so there is no
alpha at which an empty mark is both clearly visible AND dimmer than a filled one. Distinguish
filled from empty by **shape and hue** — disc vs ring, solid bar vs ringed slot — never by luminance
alone. See *Open decisions* in `SETTLED.md`; the accent's dimness is the root of it.

## Status-word empty

**Symptom** An empty state written rather than drawn: rows of "Not connected", "Forming", "No data".
Barebones and texty at the same time.
**Cause** Treating empty as an error condition to be announced rather than as data at zero.
**Fix** Reuse the section's own vocabulary at zero (§2②): unconnected inputs → filled/hollow dot rail,
forming lift → flat ghost spark, gated feature → progress-to-unlock meter. Figures show honest zeros
("0 WORKOUTS"), never a dash, never hidden. `InlineEmptyHint` is a last resort, ≤1 per lens.

## Figure wall

**Symptom** Five or more big serif figures on one screen, or dashboard-style figure rows on a detail
page. Nothing is the hero because everything is.
**Cause** Borrowing the overview toolkit on a non-overview archetype (§3).
**Fix** 2–4 `EditorialFigure`s max, one serif hero per screen (§6), everything else steps down. On a
detail page, drop the figure row entirely — it is scoped to ONE item, so context belongs in the title.

## Caption stack

**Symptom** A header, then a caption, then a hint, then an explainer — four text lines before any data.
**Cause** Each line was added to clarify the one above it.
**Fix** Prose budget (§4.3): ONE muted caption (~12 words) per section beyond the hero's context line.
A hint REPLACES the caption, never both (§12). Mechanics narration is cut, not trimmed — the state
changing IS the explanation.

## Verdict without a reading

**Symptom** A conclusion with nothing behind it: "Deload week", "Ready", "Baseline set" — no number
the user can check.
**Cause** Surfacing the engine's output and hiding its inputs.
**Fix** Show the reading beside the conclusion as row meta ("38% hard · +2", "7.2h avg"), and show
per-item readings BEFORE a verdict exists (§4.9). Below a gate the reading is progress toward it
("3 of 12 rated sets"), never "n/a". Also state verdicts in user terms, not system terms — "Ready to
coach", not "Baseline set" (§11).

## Mark echo

**Symptom** The same answer rendered twice on one screen, or the same visual repeated across lenses —
a week strip on the screen that already has a heatmap, a summary of the lens next door.
**Cause** Each lens was designed on its own and independently reached for the clearest mark.
**Fix** One home (§4.3): a fact appears once per screen; a mark that only repeats another lens's
answer is cut, not copied. Each lens stands on its OWN data.

## Machine leak

**Symptom** Raw system vocabulary reaching the screen: "2026-W27", "3 session(s)", a status enum, a
slot key, a stored paragraph of engine prose.
**Cause** Rendering a stored value directly because the row is immutable.
**Fix** Translate at render (§11) — "Week of Jun 29", the enum's word, the exercise's name, a short
derived line ("Baseline still forming · 3 of 4 sessions"). The translation lives in the UI precisely
because the stored row can't change.

## Hairline habit

**Symptom** Thin rules separating every section. The page reads as a form.
**Cause** Reaching for a divider when the real problem is not enough air.
**Fix** Sections separate by air + their mono header alone: `28 → header → 10 → content` (§7). A line
exists ONLY as data — chart threshold, floor, baseline, table rule (§1). `EditorialHairline` is for
that and nothing else.
**Seen in** Home and Stats, fixed 2026-07-08 (GYMAP-4). Other screens still carry them
(`SETTLED.md`, fix when touched).

## Boxed passive content

**Symptom** A grey card around something you can't interact with. The original sin of this codebase's
pre-editorial era.
**Cause** Using a container to group, when whitespace already groups.
**Fix** Surfaces and borders are EARNED by interactivity (§1) — can't tap it, no box. Modals keep
their surface; passive content sits directly on the page. Boxed `EmptyState`/`FirstTouchTip` are
deprecated for exactly this reason (§12).

## Debris strip

**Symptom** A row of tiny cells — a heat strip, a pip rail — that at real data volumes is a handful
of specks. Looks like dirt on the screen.
**Cause** Designing the section at its fullest state and never checking its emptiest realistic one.
**Fix** A mark needs visual mass at the data's REAL size (§12). At small counts use list rows
instead. Design each section at the emptiest state a real user will actually hit.

## Fake tap / nested tap

**Symptom** Something looks pressable and isn't; or a row and a control inside it both respond, and
which one you hit depends on a few pixels.
**Cause** Styling for emphasis (a border, a fill) on passive content; or adding a row action without
removing the row's own tap.
**Fix** Nothing looks tappable while doing nothing (§4.5); rows without a usable action render
passive. ONE tap target per row (§2③) — a row-scoped control is drawn, not independently clickable.

## Silent scale break

**Symptom** At a larger system font, text clips inside a capsule, a figure row runs off the edge, or a
name truncates to an ellipsis mid-word. Invisible at 100%.
**Cause** A fixed `.height()` on a container that holds text, `maxLines = 1` on user content, or
`fontSize =` set at the call site instead of taken from the type scale.
**Fix** Containers size to their content; 44/48dp are minimums that grow; no `maxLines = 1` on names,
notes or exercise titles; styles always from `MaterialTheme.typography` (§14). Check every touched
screen at 100% and 200%.

## Mute below the floor

**Symptom** A caption or deselected label that reads as disabled rather than secondary.
**Cause** Dimming past the ladder to make something recede.
**Fix** `muted @0.65` is a hard floor — measured 4.54:1 on Pearl, where 0.6 drops to 4.05:1 and fails
AA (§14). If something must recede further, it is the wrong element in the wrong place; cut it or move
it, don't fade it. The one exception is a text-field placeholder, which is a ghost affordance rather
than content (§5).

## Two units, one section

**Symptom** A section's header and its mark state the same fact in different units and disagree —
"4 / 7 target" printed directly above a week strip with a single cell lit. Nothing is misspelled,
nothing is misaligned, and nothing says which number to believe, so the section reads as *wrong*
without reading as *broken*. Antho pointed at this one and could not name it (2026-08-24).
**Cause** The header's numerator and the mark's data came from different fields that sound
interchangeable — `workoutsThisWeek` (sessions) against `weekDaysTrained` (days) — and the
denominator was named for the wrong one (`weeklyWorkoutTarget`, actually the program's training-DAY
count). Three sessions logged on one Monday is the case that separates them, and it is a common one.
**Fix** A mark and its caption state ONE unit. Pick the unit the mark draws, count in that unit, and
name the field for it — the rename to `weeklyTrainingDays` is what stops the bug returning, not the
call-site fix. A second, genuinely different figure (the session count) goes in the readout with its
own label, where it cannot be mistaken for the first.
