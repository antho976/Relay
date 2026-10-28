# Native client baseline

GTK 4.24.1, window 1440,900, 29 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 890.0 ms | 285.2 MiB | 0.0 | 241.9 | 285.4 MiB | 15 | 0 |
| agents-shown | 380.0 ms | 229.4 MiB | 0.0 | 243.9 | 229.4 MiB | 15 | 0 |
| start | 380.0 ms | 229.2 MiB | 25.03 | 319.1 | 256.0 MiB | 14 | 0 |
| agents-streaming | 450.0 ms | 229.2 MiB | 1.83 | 358.3 | 233.0 MiB | 14 | 0 |
| agents-burst | 470.0 ms | 230.7 MiB | 0.1 | 241.2 | 232.7 MiB | 14 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
| agents-shown | shown | 16 s | 0.0 |  |
