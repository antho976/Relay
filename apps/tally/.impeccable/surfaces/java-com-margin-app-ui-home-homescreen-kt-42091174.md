---
version: 1
slug: "java-com-margin-app-ui-home-homescreen-kt-42091174"
primary_target: "app/src/main/java/com/margin/app/ui/home/HomeScreen.kt"
related_targets: []
---

# Surface: Home (Margin)

Scope: the Home tab of the hub, plus the shell it sits in (bottom navigation on compact width,
rail on wide, one add FAB). Visitor mode: **Operate**. Audience: one person checking their own
month. Task: know the room left this month and whether the month is on pace, then log or leave.
Constraints: offline, instant reads, Avex world inherited (PRODUCT.md, Brand Commitments).

Structural candidates, ordered by resonance (seed 3a18b5e6, dealt 7 / 2 / 4, 7 leads; roll ran
degraded, no challengers; run unattended at the owner's instruction, so the lead was taken):
1 pace-first overview with a month line · 2 envelope stack · 3 today ledger · 4 calendar grid ·
5 feed-first · 6 keypad-first · **7 in / out ledger with the margin between them**.
Alternates 2 and 4 are not lost: the envelope stack is the Plan tab, the calendar grid is an
Insights lens.

## Direction contract

THESIS: The month is a two-sided ledger, money in against money out, and the margin between them
is drawn, not stated. Refuses the category default of one big balance over a card grid.

OWN-WORLD: Avex's warm near-black ground (#110F0C), serif figures, sans rows, mono uppercase
labels; one user accent (Ember default); twelve category hues as a data series. Owner-pinned
(2026-10-04) richness on top: every section is a raised 20dp panel led by a glyph tile, the
margin sits in one accent-lit hero panel, readings come as stat tiles and chips, the nav bar is a
raised Material bar with an indicator pill.

STORY: The owner opens, reads In and Out side by side, sees the margin bar and its pace tick,
understands "33 a day for 27 days", glances at the envelope that is furthest ahead of pace,
then logs with the FAB or leaves.

FIRST VIEWPORT: top bar with only a settings capsule at the end. Serif month title, sans period
line under it. The lit hero panel: wallet glyph + MARGIN label + pace verdict, the margin as the
display serif figure, "of budget, spent" line, the meter with its pace tick, chips (days left,
per day, pace). Under it IN and OUT stat tiles side by side, each against last month. Then the
week's day bars against the daily budget. FAB bottom end; raised nav bar under.

FORM: in / out ledger, candidate 7 of 7, seed key 3a18b5e6. Signature move: the pace tick, a
thin onBackground tick on every meter in the app at where an even spend would be today; fill past
the tick is the reading. Signature interaction: drag across the Insights month line to scrub
days. Motion grammar: fills draw in once (900ms DrawDecelerate), press = 0.97 bounce, all of it
collapses under Remove animations.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
