# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| burst-idle | None ms | None MiB | None | None | None MiB | None | None |
| burst-200 | None ms | None MiB | None | None | None MiB | None | None |
| burst-intent-200 | None ms | None MiB | None | None | None MiB | None | None |

## GTK thread during a session.changed burst (Agents page, 8 s window)

| row | events | instructions (user) | task-clock | per event, over idle |
|---|---:|---:|---:|---:|
| burst-idle | 0 | 24,222 | 0.1 ms |  |
| burst-200 | 200 | 92,989,492 | 14.4 ms | 464826 |
| burst-intent-200 | 200 | 8,111,877,920 | 668.4 ms | 40559268 |
