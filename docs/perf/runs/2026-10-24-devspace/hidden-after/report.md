# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 860.0 ms | 227.3 MiB | 0.0 | 0.1 | 101.1 MiB | 15 | 0 |
| agents-shown | 360.0 ms | 184.1 MiB | 0.0 | 0.3 | 150.8 MiB | 12 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 325 s | 0.0 |  |
| agents-shown | shown | 325 s | 0.55 | git.fetch 1, worktree.drift 1, worktree.list 1 |
