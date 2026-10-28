# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| burst-idle | None ms | None MiB | None | None | None MiB | None | None |
| burst-200 | None ms | None MiB | None | None | None MiB | None | None |
| burst-intent-200 | None ms | None MiB | None | None | None MiB | None | None |

## GTK thread during a session.changed burst (8 s window)

| row | page | events | instructions (user) | task-clock | per event, over idle |
|---|---|---:|---:|---:|---:|
| burst-idle | agents | 0 | 162,209 | 0.2 ms |  |
| burst-200 | agents | 200 | 104,058,009 | 15.1 ms | 519479 |
| burst-intent-200 | agents | 200 | 586,824,000 | 100.8 ms | 2933309 |
