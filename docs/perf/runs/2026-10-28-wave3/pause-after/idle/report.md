# Native client baseline

GTK 4.24.1, window 1440,900, 29 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 1270.0 ms | 284.7 MiB | 0.0 | 241.9 | 284.9 MiB | 16 | 0 |
| agents-shown | 380.0 ms | 228.3 MiB | 0.0 | 240.9 | 228.3 MiB | 15 | 0 |
| start | 370.0 ms | 229.1 MiB | 0.07 | 241.9 | 256.9 MiB | 14 | 0 |
| agents-streaming | 380.0 ms | 229.4 MiB | 1.38 | 366.2 | 233.4 MiB | 14 | 0 |
| agents-burst | 380.0 ms | 229.5 MiB | 0.03 | 242.2 | 231.3 MiB | 14 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
| agents-shown | shown | 16 s | 0.0 |  |
