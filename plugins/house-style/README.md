# House style plugin for Relay

Turn this plugin on for a project with an interface, and every agent Relay launches there designs
the way the owner wants: their taste, learned from what they chose, rejected and corrected in Relay,
Avex and Tally, on top of the fundamentals of good interface design. It is off by default; switch
it on per project from the **Plugins** page or the plugin key on the project's row in the sidebar.

## What an agent gets

| Piece | Where it lands | When |
| --- | --- | --- |
| 3 skills | `.claude/skills/house-style*/` and `.agents/skills/house-style*/` in the project root and every agent worktree, plus the provider homes for Codex | As soon as the plugin is on |
| Working rules (`instructions.md`) | The agent's session brief | Every start, wake and resume |

There is no MCP server and no op of its own: renders come from the design canvas, the project's
screenshot harness or the emulator.

## Skills

| Skill | Covers |
| --- | --- |
| `house-style` | The owner's taste (what they ask for and reject, rules that hold everywhere), precedence over project docs and generic skills, how to work with the owner. `reference/fundamentals.md` holds the general craft, `reference/failures.md` the named failure shapes with fixes, `reference/evidence.md` the owner's own words with dates |
| `house-style-native` | GTK 4 desktop (Relay's tokens, the Glass Rule, folding, headless screenshots), Android Compose (theming off stock Material, 200% font scale, touch, semantics, goldens), web pages, design-canvas mockups, game UI |
| `house-style-review` | The review: look at the real render, the checklist, ranking findings by what the owner notices first, the report format, one bounded fix round |

## How it relates to other design skills

A project's own design doc (`DESIGN.md`, Avex's `.claude/DESIGN.md` and its ledgers, `PRODUCT.md`)
still decides exact tokens, components and settled decisions. The house style fills in where the doc
is silent and overrides generic design skills (impeccable and the like) where they disagree. Those
skills' craft checks still apply; their push for bold, expressive work is kept for launch beats,
release videos and landing pages.

## Keeping it right

The taste is a snapshot of 2026-07 to 2026-10. When the owner makes a new call, add their words to
`skills/house-style/reference/evidence.md` and the rule to the skill. When a rule turns out to be
wrong, change or remove it rather than adding an exception beside it.
