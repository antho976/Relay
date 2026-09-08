# Full-app audit fixes

Base: `580519ef8d3d8ee1cc62724a7df18e585b09321c`. Work in progress.

## Recovery and startup (A01–A07)

- Resume database sidecar cleanup even when the base rename already completed.
- Persist rollback direction and retain immutable originals until recovery settles.
- Preserve committed original WAL frames when reverting a restored database.
- Copy a consistent SQL snapshot when VACUUM INTO cannot produce the backup.
- Gate Room and DataStore until recovery and restored-schema validation finish.
- Run recovery and startup preference reads on IO while showing a private-data-free waiting state.

Validation: 45 focused recovery/startup tests passed, 0 skipped. This includes real SQLite WAL
reopening, interrupted recovery retries, schema/value snapshot copying, gated preference reads
and writes, and startup with the real Hilt application graph. Full-suite and physical-device
checks are not yet complete.
