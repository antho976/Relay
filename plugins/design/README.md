# Design plugin for Relay

Turn this plugin on for a project and every agent Relay launches in it knows how to draw and revise
designs on Relay's design canvas (the project's **Design** view). It is off by default; switch it
on per project from the **Plugins** page or the plugin key on the project's row in the sidebar.

The `design.*` tools themselves are available to every agent role. What the plugin adds is the
know-how that makes their designs good: laid out with auto layout, named, edited in place, reused
through copies, colours and styles, animated with keyframes, and checked by looking at the result
with `design.render` (which needs the desktop app open). Stock photographs are a separate plugin,
Pictures.

## What an agent gets

| Piece | Where it lands | When |
| --- | --- | --- |
| The `design-canvas` skill | `.claude/skills/design-canvas/`, `.agents/skills/design-canvas/` | As soon as the plugin is on |
| Working rules (`instructions.md`) | The agent's session brief | Every start, wake and resume |

## Asking an agent from the canvas

Select layers on a design tied to a terminal and press **Ask** (or Ctrl+Enter): what you type goes
to that terminal's agent with the design's id, the page, the selected layer ids and a picture of
them. Its changes appear on the canvas as it makes them, glowing briefly; the status's Undo
button takes back its last change (each press one more), while Ctrl+Z undoes your own edits.
