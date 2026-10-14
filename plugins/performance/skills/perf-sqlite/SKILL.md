---
name: perf-sqlite
description: SQLite performance in any embedding (rusqlite, Room, Python sqlite3, others) - WAL and checkpoints, pragmas (synchronous, cache_size, mmap_size, temp_store, busy_timeout, journal_size_limit, optimize), prepared-statement caching and its capacity, indexes, covering indexes and EXPLAIN QUERY PLAN, N+1 hydration and set queries, transactions (batching, BEGIN IMMEDIATE), one connection behind a mutex, keeping slow work out of the transaction, reactive-query invalidation, migrations on large tables, and measuring where SQLite time goes (prepare vs step, callgrind, the profile hook). Use for any query, schema, store or persistence performance question.
---

# SQLite performance

In-process SQLite is fast per call — a query is a function call, not a network round trip — so
its performance problems are usually **how many statements you compile and run, and what you hold
while running them**, not raw query speed.

## 1. Settings that matter

| Setting | Recommendation | Why |
|---|---|---|
| `journal_mode=WAL` | yes, on local disk | readers never block the writer; one writer at a time; persistent setting; not on network filesystems |
| `synchronous=NORMAL` (with WAL) | yes, unless every commit must survive power loss | keeps atomicity and consistency; the last commits may roll back after a power cut; far fewer fsyncs |
| `busy_timeout` | set it (rusqlite defaults to 5 s) | without it, a contended write fails at once with `SQLITE_BUSY` |
| `cache_size` | raise for big working sets: `PRAGMA cache_size=-65536` (64 MiB) | default ~2 MiB per connection; negative means KiB |
| `temp_store=MEMORY` | usually | temp B-trees for sorts and `DISTINCT` stay off disk |
| `mmap_size` | benchmark first | helps reads only; an I/O error on the mapping kills the process with a signal |
| `journal_size_limit` | set with WAL | bounds the WAL file after checkpoints |
| `PRAGMA optimize` | `optimize=0x10002` on open for long-lived connections, then periodically and after `CREATE INDEX` | keeps planner statistics current cheaply |
| `foreign_keys=ON` | if you rely on them | off by default, per connection |

**Checkpoints**: the WAL auto-checkpoints at 1 000 pages (PASSIVE). A long-running reader
starves checkpoints and the WAL grows, slowing every read; keep read transactions short.
**Version**: a WAL-reset corruption bug affects SQLite 3.7.0–3.51.2 (fixed in 3.51.3, backported
to 3.44.6 and 3.50.7) — check what your bundled build ships.

## 2. Statements: prepare once

- Compiling SQL (`sqlite3_prepare`, the parser `sqlite3RunParser`/`yy_reduce`) often costs more
  than executing it. A profile where `prepare` dominates `step` means statements are compiled per
  call or per row.
- Use the binding's statement cache: rusqlite `prepare_cached` (default capacity **16** — raise
  it with `set_prepared_statement_cache_capacity` when a hot path uses more distinct statements,
  or the cache thrashes); Room and most ORMs cache automatically. In rusqlite,
  `Connection::query_row`/`execute` with a SQL string compile every time — use a cached statement
  on hot paths.
- **Dynamically built SQL** (filters assembled per request) defeats caching unless the text is
  stable per shape: build the same text for the same filter shape and bind the values.
- Read columns **by index** in hot row mappers if by-name lookup (`sqlite3_column_name` + `strlen`)
  shows in the profile.

## 3. Queries: set operations, not loops

- **N+1 hydration** (one list query, then several sub-queries per row) is the most common cost in
  list endpoints. Replace with one query per relation over the id set
  (`WHERE parent_id IN (…)` or a join/`json_group_array`), then assemble in memory. Cost goes
  from rows × statements to statements.
- Fetch only the columns and rows shown; `LIMIT`/keyset pagination (`WHERE id > ? ORDER BY id
  LIMIT ?`) instead of `OFFSET` on large tables; `EXISTS` instead of `COUNT(*) > 0`.
- Measure at 1× and 10× rows: linear growth in a list is expected; linear growth in a per-item
  lookup means a missing index.

## 4. Indexes and plans

- `EXPLAIN QUERY PLAN <query>`: `SCAN t` is a full scan; `SEARCH t USING INDEX` visits a subset;
  `USING COVERING INDEX` never touches the table; `USE TEMP B-TREE FOR ORDER BY` means no index
  serves the sort; `AUTOMATIC INDEX` means SQLite built a throwaway index — create it yourself.
  Read plans by eye in tests or reviews; do not parse the text (it changes between releases).
- Index columns in `WHERE`, `JOIN … ON`, `ORDER BY`, `GROUP BY`; a composite `(a, b)` serves
  queries on `a` alone. Equality columns first, then the range or sort column.
- Partial indexes (`WHERE deleted_at IS NULL`) for soft-delete tables; expression indexes for
  computed lookups.
- Every index slows writes and takes space — drop the unused ones.

## 5. Transactions and the lock you hold

- **Batch writes in one transaction**: inserts run at tens of thousands per second, but each
  commit is an fsync; one commit per row is the slow part.
- `BEGIN IMMEDIATE` for any transaction that will write — a deferred one that reads and then
  upgrades can fail with `SQLITE_BUSY` without waiting.
- **One connection behind one mutex** (a common, simple design) serializes every read with every
  write. Then the rule is absolute: nothing slow inside the transaction — no subprocess, network,
  filesystem walk, large parse or hash. Split handlers into a read phase outside the lock, a
  short locked phase that validates and writes, and post-commit work after it.
- Measure the lock, not just the query: the hot query's latency *while* a slow handler runs.
- More readers: WAL plus a small pool of read-only connections lets reads run beside the writer;
  it adds consistency questions (a read may not see an uncommitted write) — decide deliberately.

## 6. Reactive queries and invalidation

Observed queries (Room `Flow`, live queries, "refetch on change" events) re-run when any write
touches their tables, whether or not their result changed. A busy table plus several observers is
a storm. Narrow what each query reads, split hot tables from cold ones, coalesce bursts of writes
into one notification, and drop identical results before they reach the UI (`distinctUntilChanged`
or an equality check).

## 7. Schema changes

`ALTER TABLE … ADD COLUMN` is cheap; most other changes rebuild the table (create new, copy,
drop, rename) — time it on a database the size of the largest real one, inside one transaction,
before shipping. A data backfill over millions of rows runs in batches, off the startup path.

## 8. Measuring SQLite

- **callgrind** on the measured call shows `sqlite3_prepare*` vs `sqlite3_step`/`sqlite3VdbeExec`
  self cost — compile vs execute in one look.
- `sqlite3_profile`/`trace_v2` hooks (rusqlite `Connection::profile`/`trace`, Android
  `log.tag.SQLiteTime`) log each statement with its time: count statements per user action.
- `.timer on` and `.eqp on` in the `sqlite3` shell against a copy of a real database.
- `sqlite3_analyzer` for table and index sizes; `PRAGMA page_count`, `freelist_count` for bloat
  (`VACUUM` or `auto_vacuum=INCREMENTAL`).
