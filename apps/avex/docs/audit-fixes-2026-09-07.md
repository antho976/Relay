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

## Watch program resolution (A08–A09)

Phone refresh, watch display and watch logging share the customized session-plan resolver.
The mirror observes customization and program revisions. Watch logging/mirroring and reminder
and recap workers load the saved program before resolving it; unknown day keys are rejected.

Validation: 11 focused plan, current-slot and application-startup tests passed. Customized
removal, addition, ordering and prescription updates are covered with real Room. Paired-watch
cold wake and live synchronization remain device checks.
