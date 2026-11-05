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
| `house-style` | `my-list.md`, the owner's own rules, kept by hand and read first. The owner's taste (what they ask for and reject, rules that hold everywhere), precedence over project docs and generic skills, how to work with the owner, and a router to the task guides below. `reference/fundamentals.md` holds the general craft, `reference/failures.md` the named failure shapes with fixes, `reference/evidence.md` the owner's own words with dates |
| `house-style-native` | GTK 4 desktop (Relay's tokens, the Glass Rule, folding, headless screenshots), Android Compose (theming off stock Material, 200% font scale, touch, semantics, goldens), web pages, design-canvas mockups, game UI |
| `house-style-review` | The review: look at the real render, the checklist, ranking findings by what the owner notices first, the report format, one bounded fix round |

The core skill's task guides, each loaded only when its task comes up:

| Reference | From | Covers |
| --- | --- | --- |
| `motion.md` | emil-design-eng, apple-design, animate, animation-performance, unslop's motion pass | Whether to animate, the one table of curves, durations, springs and stagger, properties and jank, gestures and velocity, haptics, reduced motion, the never-ship list |
| `motion-recipes.md` | animate's recipes, emil-design-eng | Web code for press, popover, tooltip, modal, drawer, toast, accordion, stagger, hold-to-confirm, tabs, reveals, drag to dismiss |
| `motion-passes.md` | review-animations, improve-animations, find-animation-opportunities, remove-animations | Four workflows sharing one recon and rulebook |
| `motion-vocabulary.md` | animation-vocabulary | Naming a motion effect |
| `react-native.md` | animate-expo | What changes in React Native and Expo (its code recipes were dropped) |
| `unslop.md`, `landing-visuals.md`, `refs/`, `scripts/` | unslop (CC BY 4.0, Damir Sokolovsky, adapted) | The audit, the nine passes and checklist; heroes, generated visuals, code treatments, the story video |
| `stress-test.md` | break-ui | Worst-case data, failure signatures, the catalog |
| `mobile-web.md` | mobile-native | A web app that feels native on a phone |
| `libraries.md` | pick-ui-library | React library picks |
| `variations.md` | prototype | Variations built in code (its fixed picker spec was dropped) |

`fundamentals.md` also absorbed the visual values those skills shared: the spacing and radius
scales, nested radius, neutral ramps, tracking by size, chart hierarchy, icon rules, and Apple's
design foundations.

## How it relates to other design skills

A project's own design doc (`DESIGN.md`, Avex's `.claude/DESIGN.md` and its ledgers, `PRODUCT.md`)
still decides exact tokens, components and settled decisions. The house style fills in where the doc
is silent. The design skills listed above are merged here, so they are switched off in Relay's skill
store rather than installed beside it. impeccable stays separate, because it runs on its own
scripts, agents and hooks. Where it disagrees with the house style, the house style wins, its craft
checks still apply, and its push for bold, expressive work is kept for launch beats, release videos
and landing pages.

### Where the merged sources disagreed

| Question | Sources said | Settled as |
| --- | --- | --- |
| Easing on exit | Ease in when leaving (old fundamentals); ease-out always (Emil, Apple) | `--ease-out`, with the exit faster than the entrance |
| Curve | `0.23, 1, 0.32, 1` (Emil); `0.22, 1, 0.36, 1` (unslop) | Emil's three tokens |
| Large surfaces | 200–500 ms (Emil); 300–500 ms (unslop); 300–400 ms (house style) | 300–400 ms or a spring; modals 200–300 ms |
| Stagger | 30–80 ms (Emil); 40–60 ms, six items (unslop); 30–60 ms (house style) | 30–60 ms, about 300 ms in total |
| Default spring | `duration 0.5, bounce 0.2` (Emil); critically damped, bounce only after a flick (Apple) | Apple's, with Motion and Reanimated values for each |
| Reduced motion | Settle instantly (house style); gentler, not zero (Emil, Apple) | Short cross-fades that still reach the end state |
| Hover | Gated `scale(1.05)` (Emil's example); no hover scale (unslop) | A colour or surface step only |
| Stat cards | Numbers only, no sparklines (unslop); lead with a mark (house style) | Lead with a real mark, never a decorative one per card |
| Separators | Hairlines at 6–10% (unslop); air, lines are data (house style) | Air first, a hairline only where space can't separate |
| Weights | 400 and 500 (unslop); 400, 500, 600 (house style) | The house style's |
| Direction and fonts | A new character, world and typeface per product (unslop) | Only for a new product or landing page; the owner's apps keep their look and fonts |
| Translucency | Translucent chrome everywhere (Apple); avoid glass as a trend (unslop) | Only where the project uses it (Relay's Glass Rule), with Apple's rules for doing it well |

## Keeping it right

`skills/house-style/my-list.md` is the owner's own list, one rule per line, and it wins over every
other file here; only what they say in a conversation beats it. They edit it by hand. Agents never
write to it unless asked: when the owner corrects a design, the agent offers the line for them to
add. Plugins are compiled into the engine, so an edit reaches agents after Relay is rebuilt.

The taste is a snapshot of 2026-07 to 2026-10. When the owner makes a new call, add their words to
`skills/house-style/reference/evidence.md` and the rule to the skill. When a rule turns out to be
wrong, change or remove it rather than adding an exception beside it.
