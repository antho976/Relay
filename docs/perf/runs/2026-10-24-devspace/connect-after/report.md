# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| connect | None ms | None MiB | None | None | None MiB | None | None |

## bus.ping round trip, client side (200 each, µs)

| connection | p50 | p95 | max |
|---|---:|---:|---:|
| shared | 32.0 | 73.0 | 213.0 |
| own | 69.0 | 159.0 | 418.0 |
