---
version: 1
slug: "rc-main-java-com-forge-app-ui-coach-coachscreen-kt"
primary_target: "forge-android/app/src/main/java/com/forge/app/ui/coach/CoachScreen.kt"
related_targets: ["forge-android/app/src/main/java/com/forge/app/ui/coach/CoachAccount.kt","forge-android/app/src/main/java/com/forge/app/ui/coach/CoachCall.kt"]
---

# Coach — surface brief

**Scope:** the Coach tab (`ui/coach`), one page of the hub pager, also reachable by three legacy
routes. Everything the coach shows the user lives here; Settings → Coach is configuration only.

**Visitor mode:** Operate. The visitor completes a task (approve or decline this week's changes) or
reads their own record. Expression never outranks the task, the state, or a familiar affordance.

## Audience and job

Solo lifters, beginner through advanced, from one screen — depth scales with the user's own history
rather than with a mode switch. Two confirmed scenes, both real, and the structure must serve both:

- **Monday ritual.** The brief lands. Read what the coach decided, apply or skip, leave. Minutes,
  once a week, ending in a decision.
- **Idle browsing.** Between sets or on the couch, no task at all, reading their own trends because
  training is interesting. No decision owed.

Not designed for (not confirmed): in-gym pre-session checks; coach-triggered troubleshooting.

## Action and proof

The action is **Apply / Skip on a proposed change**, and it must never be the only thing the page
is good for, because most weeks have none. The proof that earns the tap is the **evidence attached
to the call itself** — that lift's strength trend, or the recovery meter and the checks that
actually fired. Confirmed non-negotiable: apply / skip / undo on every change; the evidence before
approving; the week-by-week record; goals + block + project.

## Constraints

- 200% font scale, TalkBack (value-reading descriptions on every Canvas mark), RTL, monochrome
  accent, AMOLED, and honest rendering at zero data / no Health Connect grant / no history.
- Weights through `WeightFormatter` against a tri-state unit. No hardcoded lb/kg.
- Voice: dry, imperative, "you", never "I". No exclamation marks, em dashes, hype, or ungrounded
  praise. Machine identifiers never render.
- Portrait phone only.

## Direction

**The Ledger** (surface seed key `cacfe66a`; dealt hand 5/2/1, user chose index 1). One running
account, newest first, where an open proposal and a five-week-old outcome are the same kind of
object — they differ by their node on the spine and their stamp, not by living in different
sections. Chosen over "The Readout" (one shared time axis) and "The Call Sheet" (one decision at a
time, full-bleed), both of which stay viable alternates.

Three catalog challengers were dealt and all three lost on audience identification. Kept from them:
*one measured space instead of separate views* (oscilloscope — the argument that killed the three
lenses) and *nothing disappears, it cancels* (jet-age ticket wallet — the lifecycle stamp).

**Memorable moment:** the apply. The one filled tile on the page loses its body and becomes a
stamped line on the same spine, with its two-week watch window starting under it — the user watches
a decision become a record.

## Unresolved

- **Skip is terminal, and the confirmed constraint says it should not be.** "Apply / skip / undo on
  every change" was named non-negotiable, but `CoachRepository.applyDecisionLocked` hard-returns
  unless the row is still `PROPOSED`, so a skipped call can never be reconsidered. This predates the
  ledger; un-skipping is an engine change (it would alter `TrustLedger` streak semantics, since a
  skip currently breaks the streak), so it needs a product decision, not a UI fix.
- **The accent fails its own contrast floor as a MARK on four of five choices.** `DESIGN.md` §14
  already records accent-as-TEXT failing (Red 2.42:1, Navy 2.34:1) and bans it; this surface now
  obeys that everywhere. But the same numbers are below the 3:1 a data mark owes, so every bar,
  node, meter and chart stroke is under-contrast for a user on Red or Navy. That is the app-wide
  accent palette, not this surface, and it is unaddressed everywhere.
- Undo shows on any entry carrying `undoData`, including calls whose outcome resolved weeks ago.
  Kept because "undo any time" was confirmed non-negotiable, but whether an already-judged change
  should still offer it is a product question nobody has answered.
- The record is capped at 6 weeks inline with an "and N more weeks" line. There is no destination
  behind that line yet.
- `SignalRegistry` still exists in the domain layer with nothing rendering it.
