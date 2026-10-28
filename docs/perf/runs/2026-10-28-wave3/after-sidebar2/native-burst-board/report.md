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
| burst-idle | board | 0 | 44,174 | 0.1 ms |  |
| burst-200 | board | 200 | 103,333,161 | 15.0 ms | 516445 |
| burst-intent-200 | board | 200 | 378,673,501 | 79.9 ms | 1893147 |
