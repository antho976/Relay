# Native client baseline

GTK 4.24.1, window 1440,900, 29 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 890.0 ms | 285.3 MiB | 0.0 | 240.9 | 285.3 MiB | 15 | 0 |
| agents-shown | 370.0 ms | 229.7 MiB | 0.0 | 240.9 | 229.7 MiB | 15 | 0 |
| start | 380.0 ms | 229.3 MiB | 0.03 | 241.7 | 256.6 MiB | 14 | 0 |
| agents-streaming | 360.0 ms | 228.7 MiB | 0.21 | 253.1 | 229.8 MiB | 14 | 0 |
| agents-burst | 370.0 ms | 230.1 MiB | 0.07 | 241.8 | 232.4 MiB | 14 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
| agents-shown | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
