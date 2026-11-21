# Why the pipeline was green

An external audit of `8daeb27` reported 98 findings — two Critical, twenty High — against a commit
whose CI run ([32921246217]) was green on every job: 1,127 unit tests, 44 screenshot goldens,
Android Lint on both modules at the release variant, a real R8 build, the Room migration chain on
an emulator, and a cold launch of the minified APK.

Both statements are true at the same time, and that is the interesting part. This document is the
answer to "how", written so the next person does not have to re-derive it, plus what was changed
in response.

[32921246217]: https://github.com/antho976/Avex/actions/runs/32921246217

---

## The short version

The pipeline is a **build-correctness** pipeline. It proves the app compiles, shrinks, migrates,
installs and starts, and that 1,127 assertions written by the people who wrote the code still hold.

Almost none of the 98 findings are in that space. They are in four places the pipeline has no
instrument pointed at — a second device, a second thread, a hostile input, and the gap between
what a comment promises and what the code beneath it does — plus a fifth that is worse than
uninstrumented: a path filter that lets a whole class of change through with no CI at all.

---

## Every finding, by what would have had to exist to catch it

| # | Class | Findings | Count | Could the current pipeline see it? |
| --- | --- | --- | ---: | --- |
| A | Wear / device runtime | 1, 19, 22, 24, 25, 57, 64, 65, 66 | 9 | No — nothing ever runs the watch app |
| B | Concurrency & lifecycle races | 4, 5, 6, 11, 16, 44, 47, 48, 49, 52, 58, 59, 61 | 13 | No — the suite is single-threaded by construction |
| C | Database query contracts | 7, 9, 10, 12, 23, 26 | 6 | No — 1 of 34 DAOs has behaviour tests |
| D | Hostile input | 3, 13, 27, 28 | 4 | No — every fixture is a file the app itself wrote |
| E | Build / manifest / workflow config | 17, 20, 21, 63, 67–71, 94–98 | 14 | No — CI *executes* config, it never *asserts* about it |
| F | Logic bugs in covered code | 2, 14, 15, 29–40, 45, 46, 50, 54, 55, 56 | 21 | **It looked. The tests agreed with the bug.** |
| G | UI behaviour a still cannot show | 8, 18, 41, 42, 43, 51, 53, 60, 62 | 9 | No — a golden PNG cannot press Back |
| H | Dead code | 72–93 | 22 | No — no unused-symbol analysis anywhere |

Twenty-one of ninety-eight — the largest single group — sit inside code the suite covers.

---

## A. Nine findings on hardware CI never boots

`instrumented` boots one emulator: **phone, API 34**. On it, it runs the one androidTest in the
repository (the Room migration chain) and cold-launches the minified phone APK.

The `:wear` module is compiled, linted and packaged. It is never executed — not on an emulator,
not under Robolectric. It has 16 production sources and **one** test file.

That matters more than the ratio suggests, because the Wear findings are not logic bugs. Finding 1
is that the module targets API 36 while declaring only legacy `BODY_SENSORS`. Health Services
permission enforcement is a *runtime* contract keyed to the device's API level. Compiling against
36 proves nothing about it; linting proves nothing about it; and the one emulator in the pipeline
is API 34, where the legacy permission is still the correct one. Even a Wear instrumented job at
API 34 would have reported green.

The same shape covers findings 22 (a latched "Update" state that only a delete of a live DataItem
reaches), 25 and 66 (two clocks that only disagree across a real Bluetooth link), and 64 (a buffer
that only evicts under sustained real HR delivery).

**Cost to close:** a Wear emulator job at API 36 plus permission-state tests. Real money — the
emulator is already the most expensive thing in the pipeline. Worth scoping deliberately rather
than adding by reflex.

## B. Thirteen findings on a second thread that never exists

There are 1,127 `@Test` methods. Not one of them starts a second thread.

This is structural, not accidental. Suspending code is driven through `runTest`, which runs on a
single test scheduler; the one Room harness (`RoomTestDb.inMemoryForgeDb`) is built with
`.allowMainThreadQueries()` and driven from that same scheduler. A suite built this way *cannot*
express "phone and watch both insert a set", which is the exact sentence findings 4, 5, 6 and 11
are about, nor "the user tapped LOG SET twice" (44), nor "a late Room result landed after the user
already edited the field" (58).

Every one of these is reproducible in a JVM test in a few lines — `MAX(set_index) + 1` computed
outside a transaction fails a two-coroutine race deterministically once you write the race down.
Nobody wrote the race down.

## C. Six findings in SQL nothing tests

34 DAOs. One has a behaviour suite (`LoggedSetDaoTest`, added for the strength-maximum queries).

`RoomTestDb.kt` — the shared in-memory harness — carries a doc comment describing "the DAO suites"
in the plural. There is one. The infrastructure to fix this already exists and is a single
`@RunWith(RobolectricTestRunner::class)` away from covering any DAO in the app.

The specific hazard is duplication. The exclusion contract that keeps a friend's-gym session out
of your personal bests is four SQL clauses, written out by hand in roughly a dozen queries. There
is no mechanism that notices when the thirteenth query is written with three of them. Findings 7,
10 and 12 are all exactly that: one query missing one clause, silently disagreeing with eleven
others.

## D. Four findings behind an input nobody would send

The backup/restore suites all round-trip: write an archive with the app's own exporter, read it
back, assert equality. That verifies the happy path completely and the adversarial path not at all.

Finding 3 — a `progress_photos/index.json` whose `file` field is `../../databases/forge.db` — is
invisible to a round-trip test, because the app's exporter never emits one. The ZIP *entry names*
are validated; the metadata inside the archive is trusted. The tests only ever supply metadata the
app wrote, so the trust is never tested.

## E. Fourteen findings in configuration CI runs but never reads

The pipeline treats configuration as something to execute, never as something to check:

- `release.yml` calls `ci.yml` on a `workflow_dispatch` ref and signs a *different* checkout
  (finding 21). Both jobs succeed. Nothing compares the two refs.
- `:baselineprofile` benchmarks `com.forge.app`; the app's `applicationId` is
  `com.quietsoftware.avex` (finding 20). The module builds. The profile it would generate is never
  generated in CI, so the mismatch has nothing to fail.
- `android:allowBackup` is `true` while the Privacy screen says data stays on the device
  (finding 17). No test reads either.
- Release packaging swallows a missing `mapping.txt` with `2>/dev/null || true` (finding 67).
- Four Kotlin compiler error logs containing developer-machine paths are tracked (finding 95),
  under a `.gitignore` rule that only stops *new* ones.

`guard` already proves this class of check is cheap and effective — it is why no keystore and no
`build/` directory has ever reached `main`. The rules simply stop short of the artifacts that did.

## F. Twenty-one findings the tests looked straight at and agreed with

This is the group that matters, because it is the one more tests do not fix.

The clearest specimen is finding 15. `TodayDirective` returns early when `trainedToday`, so by the
time control reaches the recovery branches, `daysSinceLast` — a *calendar-day* difference — cannot
be less than 1. Two branches guard on `daysSinceLast < 1`. They are unreachable. A user who
trained yesterday and should be told to recover is told to train.

`TodayDirectiveTest` covers both branches and passes:

```kotlin
// TodayDirectiveTest.kt
fun lowReadinessTheDayAfterTraining_suggestsMovingInstead() {
    val d = compute(s = snapshot(sessions = listOf(session(0), ...)), readiness = readiness(-4))
    assertEquals(TodayDirective.Kind.CARDIO, d.kind)   // passes
}
```

`session(0)` is a session finished **today**, and `compute` defaults `trainedToday = false`. In
production, `DirectiveRepository` derives that flag from precisely those sessions:

```kotlin
// DirectiveRepository.kt
val trainedTodayKeys = snapshot.sessions
    .filter { it.finishedAt != null && !it.isUntracked && it.startedAt >= todayStart(...) }
trainedToday = trainedTodayKeys.isNotEmpty()
```

The test constructs a state the application cannot produce, and the assertion holds inside it. The
suite is not weak here — it is *precise*, about the wrong universe. Green means "the branch does
what its author thought", not "the branch runs".

The other twenty share the shape. Findings 33 and 34 assert on windows ("yesterday's steps", "a
heavy session yesterday") that the tests supply directly rather than deriving from a clock.
Finding 39 (`asIs.size >= group.size / 2` for "at least half") passes every even-sized fixture in
the suite. Finding 54 (`> 3000` where the doc says "at or above 3000") has no fixture at 3000.
Finding 2 — swapped exercises logged in the wrong unit — is covered by tests that swap within a
unit family, which is the case where the bug does not appear.

**These are tests written from the implementation.** They were authored in the same sitting, by the
same author, as the code they check, and they encode the same misunderstanding twice. Coverage
counts them; a coverage gate would have counted them too. That is why this repository is right not
to have one.

## G. Nine findings a screenshot cannot contain

44 Roborazzi goldens render the archetype recipes at two font scales, two themes, with and without
accent. They are a genuinely strong gate for clipping, overlap and spacing — and they render
**stills**. A still cannot press the system Back button (41, 42, 43), cannot background the app and
come back to an expired lock (18), cannot tab through TalkBack's action tree (53, 62), and cannot
notice that a menu item does nothing when tapped (51).

`DesignDoctrineTest` scans source text for doctrine violations, which is a real check that catches
real things — but it reads tokens, not behaviour, and its allowlist is keyed on file paths, which
brings us to:

## H. Twenty-two findings that are just unreferenced code

No `detekt`, no `ktlint`, no unused-symbol analysis, and Android Lint's unused checks do not span
Kotlin call graphs. `ui/coach/GoalPickerDialog.kt` is 179 lines with no caller — and it has a line
in `design-allowlist.txt`, so the frozen-debt file is actively keeping a dead file's existence
documented and its deletion mildly inconvenient.

---

## What changed in response

Three things in this change, chosen because they are places the pipeline is **wrong** rather than
merely blind:

### 1. `**/*.png` no longer exempts a change from CI

`paths-ignore` listed `**/*.png` on both `push` and `pull_request`. That glob matches all 44
screenshot goldens and all 137 tracked images under `forge-android/`.

A pull request that changed only golden PNGs — the exact change `verifyRoborazziDebug` exists to
police — ran **no CI at all** and merged with a green tick supplied by the absence of any check.
The same held for every launcher and adaptive-icon asset.

The intent (don't spend 35 minutes of compute on the repository's marketing images) is preserved by
listing them where they actually live: unqualified `*.png` matches the repository root only, and
`.impeccable/**` is added for the review captures.

### 2. A malformed test result is a failure, not a footnote

`test_summary.py` handled `ET.ParseError` by appending a line to the failure *details* without
incrementing the failure *count*. A truncated `TEST-*.xml` — which means a test JVM died mid-write —
produced a summary headed **"Tests — passed"** with the broken suite listed underneath it.

Parse failures now count toward the failed total, and the script exits non-zero so a summary that
cannot be trusted fails the job that printed it. `test_summary_test.py` covers both, and runs in
`guard`.

### 3. `guard` rejects tracked tooling artifacts

The build-output rule matched `build/` and `.gradle/` only. `forge-android/.kotlin/errors/` held
four Kotlin compiler crash logs, ~70 KB, containing absolute paths from two developer machines.
`.gitignore` covers `**/.kotlin/`, which stops the next one and does nothing about these four.

They are deleted here, and `guard` now refuses `.kotlin/` so the rule is enforced rather than
merely intended.

---

## What is left, in the order it is worth doing

Each of these is a standing gap, not a bug. They are listed with what they would actually cost.

1. **DAO behaviour suites** — cheapest by a wide margin. `RoomTestDb` already exists; each suite is
   a file. Start with the queries whose contract is duplicated across a dozen call sites
   (group C, and the concurrency half of group B). *Addressed by the database PR in this series.*
2. **Concurrency fixtures.** Once a DAO suite exists, a race is `listOf(async {...}, async {...})`
   on a real dispatcher. The four highest-value ones are the exercise-slot fork, the set-index
   allocation, session finish, and the reverse-swap window.
3. **Hostile-input fixtures for backup/restore.** A handful of hand-built archives — traversal in
   the photo index, a zip bomb, a truncated entry — kept as test resources.
4. **Write the contract test before the implementation, for anything with a stated invariant.**
   Group F is not fixed by more tests of the same kind. The rule that would have caught finding 15
   is: when a comment says "X must never affect Y", the test asserts it through the *production
   caller*, not through the pure function with hand-built arguments. `DirectiveRepository` derives
   `trainedToday`; the test should too.
5. **Assertions about configuration.** A short script in `guard` can compare the baseline-profile
   target to `applicationId`, and check that `release.yml` passes one ref to every checkout. Both
   are string comparisons; neither needs a build.
6. **A Wear runtime job.** The most expensive item and the one to scope deliberately. An API 36
   Wear emulator running permission-state tests would cover group A; an API 34 one would not.
7. **Unused-symbol analysis.** `detekt` with only the unused rules enabled, plus a baseline, would
   have found most of group H. Worth doing *after* the dead code is deleted, so the baseline starts
   at zero rather than institutionalising 22 findings.

## The one-line version

A green pipeline here means "it builds, it starts, and it still does what its authors believed it
did." Of the 98 findings, 21 were inside the third clause and the other 77 were outside all three.
