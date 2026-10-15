# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| agents | 600.0 ms | 221.1 MiB | 0.0 | 0.4 | 221.1 MiB | 14 | 0 |
| board | 340.0 ms | 177.4 MiB | 0.0 | 0.2 | 180.6 MiB | 12 | 0 |
| code | 340.0 ms | 179.7 MiB | 0.0 | 0.1 | 179.9 MiB | 12 | 0 |
| notes | 330.0 ms | 176.7 MiB | 0.0 | 0.1 | 176.8 MiB | 12 | 0 |
| modules | 370.0 ms | 177.4 MiB | 0.0 | 0.1 | 177.5 MiB | 12 | 0 |
| settings | 320.0 ms | 176.8 MiB | 0.0 | 0.5 | 193.8 MiB | 13 | 0 |
| skills | 360.0 ms | 183.7 MiB | 0.0 | 0.4 | 183.8 MiB | 12 | 0 |
| dashboard | 340.0 ms | 178.3 MiB | 0.0 | 0.1 | 178.3 MiB | 12 | 0 |
| notifications | 330.0 ms | 178.1 MiB | 0.0 | 0.1 | 178.6 MiB | 12 | 0 |
| devices | 330.0 ms | 178.2 MiB | 0.0 | 0.1 | 179.3 MiB | 12 | 0 |
| launch-preview | 350.0 ms | 179.3 MiB | 0.0 | 0.1 | 183.5 MiB | 12 | 0 |
| palette | 330.0 ms | 178.1 MiB | 0.25 | 21.7 | 179.0 MiB | 12 | 0 |
| layouts | 340.0 ms | 177.6 MiB | 0.0 | 0.4 | 209.6 MiB | 12 | 0 |
| agents-streaming | 360.0 ms | 178.8 MiB | 1.25 | 104.1 | 180.1 MiB | 13 | 0 |
| agents-burst | 340.0 ms | 178.7 MiB | 0.0 | 0.1 | 180.7 MiB | 12 | 0 |
