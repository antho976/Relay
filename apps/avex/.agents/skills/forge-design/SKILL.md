---
name: forge-design
description: MANDATORY before any UI work in forge-android — creating or changing Compose screens, components, layouts, colors, fonts/typography, spacing, buttons/controls, motion/animation, charts, empty states, inputs, or any user-facing text/copy generation. Loads the binding Forge design doctrine (.claude/DESIGN.md) that all UI must follow. Trigger on words like: UI, screen, redesign, restyle, layout, design, look, theme, color, font, animation, button, page, component, copy, text.
---

# Forge design doctrine — router

**Read `.claude/skills/forge-design/SKILL.md`, then follow it.** That file is the router; this one
only forwards to it, so the two can never disagree.

This used to be a byte-for-byte copy of it with every path rewritten to a directory prefix that does
not exist in this repository — the doctrine lives under `.claude/`. An agent that loaded this copy
was told to read the binding doctrine in full before writing UI code, found nothing at the path it
was given, and proceeded with no doctrine at all. The `.claude` copy was covered by
`DoctrineSelfCheckTest`; this one was covered by nothing, which is how it drifted for as long as it
did. Both are checked now, so keep this a pointer: restore the duplicate and the tests will fail.
