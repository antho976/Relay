# Native client baseline

GTK 4.24.1, window 1440,900, 29 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 890.0 ms | 285.0 MiB | 0.0 | 240.9 | 285.3 MiB | 15 | 0 |
| agents-shown | 540.0 ms | 229.1 MiB | 0.0 | 245.9 | 229.1 MiB | 15 | 0 |
| start | 380.0 ms | 229.4 MiB | 29.62 | 327.8 | 256.6 MiB | 14 | 0 |
| agents-streaming | 370.0 ms | 229.1 MiB | 1.17 | 385.3 | 233.4 MiB | 14 | 0 |
| agents-burst | 380.0 ms | 229.6 MiB | 0.1 | 241.8 | 231.6 MiB | 14 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
| agents-shown | shown | 16 s | 0.0 |  |
