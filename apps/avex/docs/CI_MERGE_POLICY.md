# What has to be true before a change reaches `main`

Written 2026-09-02, after two pull requests merged with their tests never having run against them
and left `main` unable to compile. This is the half of the fix that lives outside the workflow file.

## What happened

PR #165 and PR #166 both merged while their `Verify (JVM)` job had not started. `Guard` is the
cheap job — wrapper checksum, no committed secrets, no committed build output — and it finishes in
about thirty seconds; `Verify` is the fifteen-minute one that actually compiles the app and runs the
tests. A green `Guard` was enough to satisfy the merge, so both PRs went in on the strength of a
check that never opens a Kotlin file.

Each then broke the build on `main`:

- [#165](https://github.com/antho976/Avex/actions/runs/33646602275) — `TrophiesViewModel` did not compile.
- [#166](https://github.com/antho976/Avex/actions/runs/33658684526) — `OverviewViewModel` did not compile.

`Verify` for #166 did eventually start, and failed before compiling anything: it checked out
`refs/pull/166/merge`, a transient ref GitHub deletes on merge, which by then was gone.

## What is fixed in this repository

`.github/workflows/ci.yml` now checks out an **immutable commit** in all three jobs —
`github.event.pull_request.merge_commit_sha` on a pull request, `github.sha` otherwise — so a job
that starts after its PR has merged still tests the tree it was queued for instead of failing on a
missing ref. A release call naming a tag still wins, as before.

A failed `Verify` also ends by printing what failed (compile errors, then failing test names), so a
red run is readable from the log without downloading an artifact.

## What still has to be configured on GitHub

None of this can be committed — it is repository settings, and it needs an admin:

1. **Protect `main`.** At review time `main` had no branch protection at all
   (`GET /repos/antho976/Avex/branches/main/protection` returned 404), which is why a PR could merge
   with a job unstarted.
2. **Require these checks to pass, not merely to have been attempted:**
   - `Guard`
   - `Verify (JVM)`
   - `Instrumented (emulator)`
3. **Require branches to be up to date before merging**, so a green run describes the tree that
   actually lands.
4. **Do not allow a skipped required job to count as passed.** A required check that is `skipped`
   satisfies protection by default; the three jobs above run on every ordinary pull request, so a
   skip means the workflow did not run and the merge should wait.

Until 1–4 exist, the workflow is a report rather than a gate, and this document is the record of
which of the two it currently is.
