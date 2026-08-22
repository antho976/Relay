# Product

<!-- impeccable:product-schema 1 -->

## Platform

android

## Users

Solo lifters training in a gym, spanning beginners through advanced. The Coach surface must
serve someone in their second week and someone in their sixth year from the same screen, so
depth scales with the user's own history rather than with a mode switch or a skill setting.

Two confirmed usage scenes, both real:

- **The Monday ritual.** The weekly brief lands; the user reads what the coach decided, applies
  or skips it, and leaves. Minutes, once a week, with a decision at the end.
- **Idle browsing.** Between sets or on the couch, with no task at all: reading their own trends
  because training is interesting. No decision, and no obligation to produce one.

Not confirmed, so not designed for: in-gym pre-session checks, and coach-triggered
troubleshooting when something feels wrong.

## Product Purpose

Avex (Android package `com.forge.app`, module `forge-android`) is a fully offline personal gym
coach: it generates a training program around the user's equipment and schedule, logs their
sessions, and then adapts the program week by week from what they actually lifted.

The Coach surface is where that adaptation becomes visible and consented to. Its job is to make
one week's proposed changes understandable and approvable, and to stand behind them afterwards
with the evidence and the record.

Success is the user understanding why their training changed, agreeing to it, and being able to
check later whether it worked.

## Positioning

**The coach proposes, the user approves, and both halves are on the record.** The weekly pass
never silently rewrites the program: every change is a proposal carrying its own evidence, apply
and skip are one tap, undo stays available, and a two-week watch window afterwards records
whether the change actually worked. Autopilot is earned per change type by that type's own track
record, not granted by a setting.

A neighboring app can copy automatic progression. It cannot truthfully copy the consent trail:
the proposal, its evidence, its outcome, and the week-by-week ledger of what the coach got right.

Second, it is genuinely offline: no account, no servers, and the app holds no internet
permission at all. Every reading the coach uses is derived on-device from the user's own logs
plus optional Health Connect data.

## Operating Context

- The weekly pass runs on Monday and produces a **brief**: a set of `CoachDecision` rows, each
  with a summary, a reason, a target lift, and a type (progression, deload, volume, rotation,
  rep-range shift).
- A decision's lifecycle: proposed → applied / skipped → (folded into baseline | reverted), with
  an outcome of pending / ok / failed resolved over a **14-day watch window**.
- The coach holds rather than calls when it lacks a baseline; a new account needs a minimum
  number of logged sessions before its first real call.
- Longer arcs run underneath the weekly cycle: **goals** (a target per lift or muscle, with a
  live reading and an ETA), a **training block** with four phases, and at most one **project**
  (a single named lever with a plan and a finish line).
- Recovery inputs are optional and frequently absent: Health Connect sleep and resting heart
  rate, HRV, plus on-device signals (session ratings, rest-day flags, soreness). Health Connect
  exposes data presence, never capability, so an absent signal is never "unsupported".
- Freestyle logging mode leaves no program to coach against; the Coach surface has nothing to
  show and says so.

## Capabilities and Constraints

- Jetpack Compose, Material 3, Hilt, single-activity. Coach is one page of a hub pager, reached
  as a tab rather than a route in normal use.
- All data is local (Room). Reads are instant — there is no network latency to design loading
  states around, and no spinners.
- The screen must survive 200% font scale, TalkBack, RTL, monochrome (accent disabled), and
  AMOLED, and must render honestly with zero data, no Health Connect grant, and no history.
- Weights render through `WeightFormatter` against a tri-state unit (lb / kg / st). No hardcoded
  units anywhere.
- Existing state surface (`CoachViewModel.UiState`): brief, watch, timeline, per-slot estimated
  1RM series, Health Connect series, days to next brief, goals and goal conflicts, signal slot
  registry, training block, project and project proposal, learned personal profile.

### Must survive the Coach redesign (user-confirmed, 2026-08-20)

1. Apply / skip / undo on every proposed change.
2. The evidence behind a call — the trend or reading the coach decided from, visible before the
   user approves. Removing it would make the coach a black box.
3. The week-by-week record of what the coach did and whether it worked.
4. The longer-arc planning layer: goals, training block, and project.

## Brand Commitments

- The product name renders as **Avex**; `Forge` survives as the package and module name and in
  the internal rank ladder, not as the user-facing brand.
- The launch wordmark "• Avex" is a cold-launch beat only, never chrome.
- **Voice (binding):** dry, specific, and grounded in the user's own numbers. The coach speaks
  imperative and "you", never "I", and never names itself. Banned in any rendered string:
  exclamation marks, em dashes, hype or bro-speak, poster clichés, and praise the data does not
  support. Generated lines vary with the numbers instead of repeating one generic cue.
- Machine identifiers never render: week ids become human dates, status enums become words, slot
  keys become exercise names.
- User-chosen accent color (Ember default, four alternates) with a monochrome option, plus dark
  and AMOLED themes. Any surface must read correctly under every one of those choices.

## Evidence on Hand

- Real product content: `.claude/FEATURES.md` (shipped feature inventory), `.claude/CHANGELOG.md`.
- Real design history: `.claude/DESIGN.md` and the satellites in `.claude/design/`
  (`MAP.md`, `SETTLED.md`, `FAILURES.md`, `AUDIT.md`, `DECISIONS.md`) — a removals ledger and a
  named-failures list that future work must consult before re-adding anything.
- Real implementation: `forge-android/app/src/main/java/com/forge/app/ui/coach/` and the domain
  layer under `domain/coach/` and `domain/adapt/`.
- Store assets at the repo root (icon, invite banner, app-icon family renders).
- **No fabricated evidence exists or may be invented:** no testimonials, no user counts, no
  benchmarks, no pricing, no coaching claims the engine does not actually compute.

## Product Principles

1. **The coach proposes, the user disposes.** Nothing changes the program without consent, and
   consent is always reversible.
2. **Show the reading, not just the verdict.** A conclusion travels with the number it came
   from; below a gate, show progress toward the gate rather than "not enough data".
3. **Serve both ends of the ladder from one screen.** Depth grows with the user's history; a
   beginner is never shown an empty expert instrument, and an advanced user is never capped.
4. **Honest at zero.** Every state — no data, no grant, no history, no call this week — is
   drawn, not hidden and not apologized for.
5. **Offline is a promise, not a limitation.** Every reading is derived from the user's own data
   on their own device.

## Accessibility & Inclusion

Confirmed product requirements: 200% font scale without clipping or lost content; TalkBack
support including value-reading descriptions on every Canvas mark; touch targets from padding at
48dp minimum; RTL-correct layout; a monochrome mode for users who disable the accent; and text
contrast at 4.5:1 or better against the app's near-black ground.
