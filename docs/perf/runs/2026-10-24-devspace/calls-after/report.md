# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| calls-code-0 | None ms | None MiB | None | None | None MiB | None | None |
| calls-code-1 | None ms | None MiB | None | None | None MiB | None | None |
| calls-code-2 | None ms | None MiB | None | None | None MiB | None | None |

## Requests from launch to 10 s

| run | page | total | by op |
|---|---|---:|---|
| calls-code-0 | code | 62 | app.status 1, arbiter.summary 1, bus.subscribe 1, design.render.serve 1, device.list 1, file.tree 3, git.status 4, guardrail.requests.list 1, notes.list 1, notify.list 3, project.list 1, provider.list 1, session.attach 6, session.list 1, session.resize 12, settings.get 14, settings.set 2, task.list 1, ui.page.switch 1, usage.get 1, workspace.list 1, worktree.drift 1, worktree.list 3 |
| calls-code-1 | code | 50 | app.status 1, arbiter.summary 1, bus.subscribe 1, design.render.serve 1, device.list 1, file.tree 3, git.status 3, guardrail.requests.list 1, notes.list 1, notify.list 3, project.list 1, provider.list 1, session.attach 6, session.list 1, settings.get 14, settings.set 3, task.list 1, ui.page.switch 1, usage.get 1, workspace.list 1, worktree.drift 1, worktree.list 3 |
| calls-code-2 | code | 45 | app.status 1, arbiter.summary 1, bus.subscribe 1, design.render.serve 1, device.list 1, file.tree 3, git.status 4, guardrail.requests.list 1, notes.list 1, notify.list 3, project.list 1, provider.list 1, session.list 1, settings.get 14, settings.set 3, task.list 1, ui.page.switch 1, usage.get 1, workspace.list 1, worktree.drift 1, worktree.list 3 |
