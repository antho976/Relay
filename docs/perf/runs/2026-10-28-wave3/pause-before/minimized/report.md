# Native client baseline

GTK 4.24.1, window 1440,900, 29 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 910.0 ms | 282.1 MiB | 0.0 | 241.9 | 282.3 MiB | 15 | 0 |
| agents-shown | 370.0 ms | 229.3 MiB | 0.0 | 241.9 | 229.4 MiB | 15 | 0 |
| start | 360.0 ms | 229.4 MiB | 0.07 | 241.8 | 256.5 MiB | 14 | 0 |
| agents-streaming | 370.0 ms | 229.6 MiB | 0.24 | 253.3 | 230.2 MiB | 14 | 0 |
| agents-burst | 370.0 ms | 230.2 MiB | 0.07 | 241.8 | 232.1 MiB | 14 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
| agents-shown | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
