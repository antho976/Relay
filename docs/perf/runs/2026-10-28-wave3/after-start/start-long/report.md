# Native client baseline

GTK 4.24.1, window 1440,900, 29 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 1460.0 ms | 286.5 MiB | 0.0 | 237.7 | 286.6 MiB | 15 | 0 |
| agents-shown | 380.0 ms | 229.7 MiB | 0.0 | 242.4 | 229.7 MiB | 15 | 0 |
| start | 570.0 ms | 230.8 MiB | 30.24 | 326.3 | 257.1 MiB | 14 | 0 |
| agents-streaming | 540.0 ms | 229.1 MiB | 3.1 | 409.5 | 233.2 MiB | 14 | 0 |
| agents-burst | 610.0 ms | 230.2 MiB | 0.17 | 241.9 | 154.5 MiB | 14 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
| agents-shown | shown | 16 s | 0.0 |  |
