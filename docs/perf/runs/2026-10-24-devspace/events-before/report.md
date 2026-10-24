# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| code-events-quiet | None ms | None MiB | None | None | None MiB | None | None |
| code-events | None ms | None MiB | None | None | None MiB | None | None |

## Path lookups on the GTK thread while files change (Code page)

| row | files changed | window | stat/readlink calls, main thread | per change |
|---|---:|---:|---:|---:|
| code-events-quiet | 0 | 2.5 s | 0 | None |
| code-events | 100 | 7.53 s | 272 | 2.7 |
