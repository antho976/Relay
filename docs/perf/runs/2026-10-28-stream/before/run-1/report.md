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
| stream-idle | 0 | 3.0 s | 68,498 | 0.1 ms | 0 | None | None | None | 0 |
| stream-2000 | 2000 | 2.61 s | 8,531,792,174 | 624.3 ms | 103 | 2.3 | 7.2 | 9.8 | 0 |
| stream-8000 | 8000 | 10.14 s | 91,669,296,890 | 6253.0 ms | 442 | 13.1 | 27.4 | 32.6 | 142 |
