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
4. **Do not let a skipped job stand in for a build.** GitHub counts a required check skipped by its
   own `if:` as passed, and has no setting to change that. So the only skip is a deliberate one:
   `pull_request` has no `paths-ignore` (a workflow that never starts never reports, and a
   docs-only PR would wait forever for its checks), and `Guard` runs `.github/scripts/ci_scope.py`
   on every PR. `Verify (JVM)` and `Instrumented (emulator)` are skipped only when the PR's merge
   commit changes nothing but the paths the push trigger ignores (docs, root media, `PRODUCT.md`,
   `README.md`). Any other file, an empty diff or a helper error builds.

## Status

**2026-09-26:** `main` was red from 09-21 to 09-26 (CI runs 308–325,
`DesignDoctrineTest.noEmDashesInRenderedStrings`) while PRs #187–#194 merged, so the migration and
smoke-launch job was skipped for five days (release audit 2026-09-26, `docs/audits/2026-09-26/12`).
Item 4 is done in the workflow (`310ef05`). Items 1–3 are NOT yet applied: at the end of that day
`GET /repos/antho976/Avex/branches/main/protection` still returned 404. An earlier version of this
section said `main` was protected; it was not.

The intended settings, applied by a repository admin:

```sh
gh api -X PUT repos/antho976/Avex/branches/main/protection --input - <<'EOF'
{
  "required_status_checks": {
    "strict": true,
    "contexts": ["Guard", "Verify (JVM)", "Instrumented (emulator)"]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": null,
  "restrictions": null
}
EOF
```

Admins are not included in enforcement, so an emergency merge stays possible, but only as an
explicit `gh pr merge --admin` or the bypass box in the web UI; an ordinary merge waits for green.
The emulator job fails now and then on runner flakes (an install racing the launch), so expect
the occasional re-run.

Check it with `gh api repos/antho976/Avex/branches/main/protection/required_status_checks`, and
update this section once it answers.
