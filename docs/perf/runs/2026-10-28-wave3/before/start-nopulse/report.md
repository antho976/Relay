# Native client baseline

GTK 4.24.1, window 1440,900, 29 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 1470.0 ms | 288.5 MiB | 0.0 | 241.9 | 288.6 MiB | 16 | 0 |
| agents-shown | 360.0 ms | 229.2 MiB | 1.0 | 241.9 | 229.2 MiB | 15 | 0 |
| start | 360.0 ms | 229.1 MiB | 0.31 | 301.9 | 257.1 MiB | 14 | 0 |
| agents-streaming | 440.0 ms | 227.9 MiB | 2.14 | 405.2 | 231.8 MiB | 14 | 0 |
| agents-burst | 400.0 ms | 229.4 MiB | 0.03 | 241.9 | 231.2 MiB | 14 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
| agents-shown | shown | 16 s | 0.0 |  |
