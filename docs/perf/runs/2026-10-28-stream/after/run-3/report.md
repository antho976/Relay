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
| stream-idle | 0 | 3.0 s | 55,639 | 0.1 ms | 0 | None | None | None | 0 |
| stream-2000 | 2000 | 2.61 s | 1,533,134,521 | 169.3 ms | 162 | 0.9 | 1.4 | 9.5 | 0 |
| stream-8000 | 8000 | 10.14 s | 6,245,358,704 | 625.0 ms | 555 | 0.9 | 1.4 | 31.6 | 1 |
