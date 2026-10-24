# Native client baseline

GTK 4.24.1, window 1440,900, 8 s per page, perf off.

| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |
|---|---:|---:|---:|---:|---:|---:|---:|
| board-400 | None ms | None MiB | None | None | None MiB | None | None |
| board-2000 | None ms | None MiB | None | None | None MiB | None | None |

## Board opening: navigate → first card drawn and laid out

| row | tasks | cold mount, median of 3 launches | each launch's cold | reopen, median | reopen, max |
|---|---:|---:|---|---:|---:|
| board-400 | 400 | 351.7 ms | 344, 352, 383 | 7.1 ms | 15.4 ms |
| board-2000 | 2000 | 576.9 ms | 570, 577, 751 | 12.3 ms | 14.5 ms |
