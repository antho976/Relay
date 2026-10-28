# Native client baseline

GTK 4.24.1, window 1440,900, 29 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents-hidden | 1450.0 ms | 284.6 MiB | 0.0 | 242.9 | 284.7 MiB | 16 | 0 |
| agents-shown | 370.0 ms | 230.1 MiB | 0.0 | 240.8 | 230.1 MiB | 15 | 0 |
| start | 590.0 ms | 229.7 MiB | 28.72 | 367.5 | 256.4 MiB | 14 | 0 |
| agents-streaming | 540.0 ms | 231.1 MiB | 1.69 | 377.0 | 234.9 MiB | 14 | 0 |
| agents-burst | 560.0 ms | 230.8 MiB | 0.1 | 241.8 | 234.2 MiB | 14 | 0 |

## Requests the client sends with the window minimized, and on screen

| row | window state | counted | requests/min | by op |
|---|---|---:|---:|---|
| agents-hidden | Some(ToplevelState(TOP_RESIZABLE | RIGHT_RESIZABLE | BOTTOM_RESIZABLE | LEFT_RESIZABLE | SUSPENDED)) | 16 s | 0.0 |  |
| agents-shown | shown | 16 s | 0.0 |  |
