# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 640.0 ms | 228.9 MiB | 0.0 | 0.2 | 71.0 MiB | 14 | 1 |
| agents-shown | 490.0 ms | 184.0 MiB | 0.0 | 0.2 | 119.7 MiB | 12 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 325 s | 0.74 | git.fetch 1, worktree.drift 2, worktree.list 1 |
| agents-shown | shown | 325 s | 12.18 | app.status 1, arbiter.summary 1, bus.subscribe 1, design.render.serve 1, device.list 1, file.tree 3, git.fetch 1, git.status 4, guardrail.requests.list 1, notes.list 1, notify.list 3, project.list 1, provider.list 1, session.attach 6, session.list 1, session.resize 11, settings.get 14, settings.set 3, task.list 1, ui.page.switch 1, usage.get 1, workspace.list 1, worktree.drift 2, worktree.list 5 |
