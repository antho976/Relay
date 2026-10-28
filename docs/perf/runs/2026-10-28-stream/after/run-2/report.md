# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| stream-idle | None ms | None MiB | None | None | None MiB | None | None |
| stream-2000 | None ms | None MiB | None | None | None MiB | None | None |
| stream-8000 | None ms | None MiB | None | None | None MiB | None | None |

## GTK thread while a reply streams into the assistant drawer (Board page)

| row | characters | stream | instructions (user) | task-clock | frames | frame_end p50 | p95 | max | over 16.7 ms |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| stream-idle | 0 | 3.0 s | 69,602 | 0.1 ms | 0 | None | None | None | 0 |
| stream-2000 | 2000 | 2.61 s | 1,543,664,462 | 171.8 ms | 104 | 0.8 | 1.3 | 9.6 | 0 |
| stream-8000 | 8000 | 10.13 s | 6,234,398,148 | 627.4 ms | 614 | 0.9 | 1.3 | 31.9 | 1 |
