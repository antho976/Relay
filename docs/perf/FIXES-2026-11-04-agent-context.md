# Fixes 2026-11-04: what an agent reads

What Relay hands an agent costs tokens on every turn (the system prompt) or once per call (a
tool's answer). Measured in bytes of text as the agent receives it (≈ 4 bytes a token), on the
owner's dev instance with the Design, House style, Performance, Pictures and Video plugins on
for RelayV4. Bus latency was measured too and is not the problem: `bus.ping`,
`session.bootstrap`, `bus.ops` and `task.list` answer in 1–3 ms, the pre-tool hook in 5–11 ms.

## Every turn: the system prompt Relay appends

`.relay/sessions/<name>/role-instructions.md` (role instructions, then the compact brief).

| | bytes |
|---|---:|
| before | 32 495 |
| after (reconstructed from the new texts on the same session's file) | **8 982** (−72 %) |

What went:

- **Plugin skills listed twice** (13.9 KB). `plugins::brief_section` listed every plugin skill
  with its whole description, which Claude Code's own skill list shows already. The plugin's
  instructions name each skill (a test holds every bundled plugin to that), and the brief says
  once where the folders are.
- **Plugin how-to in every session** (12.2 KB → 3.3 KB). Each plugin's instructions are now its
  trigger and the rules that must hold before a skill is loaded (ask before touching a design,
  never post a clip, search words leave the machine, credits cost money). The rest was in the
  skills already, except the Performance rules, which moved into `perf-fundamentals` as its
  section 0, word for word. A test keeps these five under 1 100 bytes.
- **Comms said twice**: the compact brief's `## Comms` repeated the role instructions it always
  travels behind.

## Once a call: answers through the MCP door

| Answer (`relay mcp`, tool result text) | before | after |
|---|---:|---:|
| `bus.ops` (the discovery hint sent every agent here) | 83 760 | **24 218** — the 201 callable ops, name and summary |
| `session.bootstrap` | 6 076 | **2 060** — no `can_call` (the tool list again), `discovery`, `comms` (the prompt) |
| `design.get` whole, design 13 (3 238 nodes) | 554 172 | **1 580** — its pages and `whole_too_big`, past 80 KB |
| `design.get` whole, design 18 (435 nodes) | 74 177 | 74 177 (under the cap) |

| Answer (CLI) | before | after |
|---|---:|---:|
| `relay ops --mine` | 46 175 (all 271 rows) | **22 949** (callable only) |
| `relay q task.list {"summary":true}` piped | 775 | **506** (one line when stdout is not a terminal) |

The engine's own answers are unchanged: the desktop app, the phone and `relay q` as a person
see what they saw. The door shapes `bus.ops` only when no `actor` is asked about.

## Not changed, and why

- The pre-tool hook refuses `$RELAY_BIN q $op` (an op in a variable) and a heredoc fed to an
  interpreter whose text says `` `relay q` ``. Both are the self-approval rule reading what it
  cannot see as the worst case; loosening it would let an agent answer its own guardrail. It
  costs a turn when it fires.
- The ~20 internal references in tool schemas (`SPEC §6`, `(D118)`) come to ~200 bytes.
- Next, not in this change: shorter skill descriptions in front matter (~2k tokens a session),
  and `design.apply`'s 61 KB tool schema (40 KB of it the node model `design-canvas` teaches).
