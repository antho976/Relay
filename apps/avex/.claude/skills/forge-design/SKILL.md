---
name: forge-design
description: MANDATORY before any UI work in forge-android — creating or changing Compose screens, components, layouts, colors, fonts/typography, spacing, buttons/controls, motion/animation, charts, empty states, inputs, or any user-facing text/copy generation. Loads the binding Forge design doctrine (.claude/DESIGN.md) that all UI must follow. Trigger on words like: UI, screen, redesign, restyle, layout, design, look, theme, color, font, animation, button, page, component, copy, text.
---

# Forge design doctrine — loader

1. **Read `.claude/DESIGN.md` in full, now, before writing or planning any UI code.** It is
   binding: tokens, the accent-alpha ladder, the three type voices, spacing rhythm, screen
   archetypes, component inventory, control sizes, motion, voice/copy rules, empty states,
   inputs/loading/feedback, settled decisions, and the pre-finish checklist.

2. Non-negotiables that get missed most often:
   - Pick the screen's **archetype** (§3) first and use only its toolkit — never the full
     editorial kit on settings/forms.
   - **Alphas only from the ladder** (§5); colors via `MaterialTheme.colorScheme`.
   - **No prose behind taps, no bare hyperlinks, nothing fake-tappable** (§4).
   - **Top bar = `• Forge` wordmark + `←` + ≤1 action, never the screen's own name** (§2/§4.7) —
     the name is a serif content hero or nothing. **Serif titles/verdicts take no terminal
     period** — "Coach", "Baseline set", not "Coach." (§11).
   - **Prose budget** (§4.4): sections lead with data (figure/bar/chart/tag), max ONE ~12-word
     caption line each; cut mechanics narration; never repeat a line across a screen's lenses.
   - Reuse `ui/common/` primitives (§8); trim control sizes (switch 40×24, capsules 44dp).
   - Generated text: dry, data-grounded, imperative + "you", no exclamation marks (§11).
   - **Empty = data at zero, DRAWN not written** (§12): every data section leads with a MARK
     (meter/bar/sparkline/dot row) at zero/ghost — never status words; forming→ghost spark,
     unconnected→hollow dots, threshold→progress-to-unlock meter; collapse repeated empty rows to
     one; the written hint is a last resort capped at ONE per lens; never caption+hint or a bare
     text row. A data screen must carry ≥1 visual mark that works at zero (§3).
   - **Two-shot build flow**: overview screen first → Antho device-checks → then sub-UIs (§4.8).

3. Before calling the work done, run the §15 checklist against every touched screen.

4. If a design decision is made or changed during the task, **update `.claude/DESIGN.md` in the
   same turn** — the doc stays the single source of truth (keep it compact; it is deliberately
   dense at ~200 lines).
