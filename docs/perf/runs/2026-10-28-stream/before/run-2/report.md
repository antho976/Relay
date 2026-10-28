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
| stream-idle | 0 | 3.0 s | 67,287 | 0.1 ms | 0 | None | None | None | 0 |
| stream-2000 | 2000 | 2.61 s | 8,557,328,738 | 640.9 ms | 103 | 2.4 | 7.1 | 9.5 | 0 |
| stream-8000 | 8000 | 10.14 s | 97,154,205,788 | 6696.5 ms | 460 | 13.4 | 28.0 | 32.3 | 167 |
