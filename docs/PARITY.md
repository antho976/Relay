# Relay-2 native parity coverage

Reference: Relay-2 `1745dd3f68b7bc786f48142ad7da591537aeb057`.
Native stack: Rust, GTK4, VTE and GtkSourceView, matching Relay-V3.
The archived V3 roadmap remains unchanged; its completion marks describe V3.

## Implemented native workflows

| Relay-2 surface | Native implementation |
| --- | --- |
| Shell and project registry | Compact title bar, sidebar navigation, grouped workspaces, project selection, registration, rename, pinning, base/build/run settings and confirmed registry removal |
| Terminal wall | Retained VTE widgets; Grid, Focus and Review modes; resizable columns; ordering; file rail; named layout save/apply/delete |
| Launch and review groups | 1–11 solo profiles or one/two builders sharing a reviewer; provider/model/effort, worktree, assignment, write/UI rights and task queue controls; allocations and queues precede spawning; reviewer starts first |
| Session lifecycle | Start, park, wake, resume, fresh restart, brief inspection, pre-spawn configuration, confirmed close and optional worktree/build cleanup |
| Coordination and safety | Existing engine role boundaries, claims, mailbox priority, review routing, task approval and exact held-operation inspection; agent permission and human task approval remain distinct |
| Board | Five columns, search/type/priority filtering, drag moves, task editor, metadata, labels, subtasks, dependencies, duplicates, changelog, commit links, attachments, dispatch and approval |
| Modules and Notes | Module editing/archive, changelog generation, separate retained Notes workspace, searchable library, persistent editor tabs, formatting, find/replace and dirty-close protection; former Plan documents are ordinary notes |
| Project Files and Git | Primary/agent checkout selection, expandable tree, content search, file creation/rename/trash/restore, drag/move, highlighted editing, find/replace, dirty protection, worktree diff, staging, commits, history, branch management, fetch/push/PR and disposable merge tests; separate Code navigation removed |
| Dashboard and notifications | Cross-project overview, correct project/task/session destinations, read acknowledgements, category settings, generated audio patterns, volume and preview |
| Skills | Local editing, GitHub installation and replacement, source refresh, per-project enablement and deletion |
| Settings and usage | Relay-2 palettes and SVG geometry, wallpaper library, opacity/dimming/contrast, terminal font, configurable Relay-2 shortcuts, provider/device paths, parking, guardrail configuration, backups and provider usage panels |
| Android | Device/AVD listing, boot/create, selected-worktree run/build, APK/AAB, optional publishing, signing configuration, bounded logs and native mirror/control/capture |
| Plugins | Bundled plugins switched on per project (D159); the Unreal Engine plugin ships 20 skills, agent rules, docs and the `relay unreal-mcp` server; the Blender plugin ships 6 skills and `relay blender-mcp` |

App-owned editors, confirmations, registry controls, usage and build logs stay
inside the main window. Task/module details use the content page; command and
layout menus use compact in-app surfaces. The launch sheet uses the Relay-2
780px width, provider marks, choice cards and fixed footer. Only Notes and the emulator/mirror detach; native file
pickers still use the desktop file chooser. Dirty task, module, skill and note
drafts block dismissal and application closure until saved or discarded.

## Reliability changes

- Task, note and module updates accept optional expected editable fields, checked
  in the write transaction. Stale drafts are rejected without changing the row.
- File writes accept the expected original SHA-256, checked inside the engine
  handler. This closes the competing bus-writer gap; unrelated external filesystem
  writers cannot be made atomic by this API.
- Session allocation persists assignments before spawning, so partially launched
  groups can recover from the wall without losing their prompt.
- Native mirror connections own their capture lifetime. The engine reports the
  mirror's state on that same stream (starting, running with the real picture size,
  then stopped, failed with its code, or lost), so a window never waits on silence.
  Input is answered from memory, queued moves collapse into the newest, and downs
  and ups are never dropped; overflow no longer stops capture. The client decodes
  in process (libavcodec, low delay, one access unit per packet, one context across
  rotations) and hands GTK the YUV planes, which it converts on the GPU; the
  `ffmpeg` child and its RGB pipe are gone. `scripts/mirror-latency/` measures it.
- Device watchers are leased per socket. Duplicate requests retain all schema and
  actor validation; disconnect releases only that connection's watcher.
- Closing Notes hides its retained window and preserves drafts. The main
  window destroys it only after dirty-draft checks. Drafts remain project-qualified.

## Validation and remaining acceptance

[VERIFICATION.md](VERIFICATION.md) records the checks and their limits. Core
workflows have native implementations; this is not a claim of pixel identity,
release readiness or physical-device signoff.

Remaining acceptance covers real Claude/Codex TUI sessions, physical Android
build/signing/mirror behavior, external publishing, notification audio on the
user's sound setup, accessibility and prolonged performance. No real provider,
release upload, signing identity or user-store migration was used for smoke tests.

The first-run surface now groups workspace creation, provider detection and
local/GitHub repository selection. Git history draws branch and merge lanes
from commit parent SHAs. Advanced docking trees, packaging, migration and
subsequent product features stay in the imported roadmap.

The September 4 UI pass uses the reference's bundled Fira fonts, SVG geometry,
palette, compact shell dimensions and page compositions. Native screenshots
and widget bounds are recorded in `.impeccable/review/`; rendered reference
screenshots are in its `reference/` directory. Smoke checks assert the 42px
top bar, 24px status bar, 28px Files rail and 26px terminal headers, as well as
the requested visible page. These measured surfaces do not establish pixel
identity across every content state, display scale or compositor.

The subsequent chat-roadmap implementation is recorded in
[native-roadmap-results.md](../outputs/native-roadmap-results.md). It supersedes
the original Files rail, Plan navigation, and in-app Notes composition. The
42px main top bar, 24px status bar and 26px terminal headers remain measured.
