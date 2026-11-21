# 00 — CI, build & release-gate findings

Source: GitHub Actions history for `antho976/Avex`, workflow `ci.yml`, branch `main`.
Gathered directly from the Actions API, not inferred.

## [CRITICAL] CI has been red on `main` for ~25 consecutive runs — the release is unverified

**Evidence:** Last successful run on `main` was **run #102 (2026-08-22T16:44Z, sha `603d066`)**.
Every subsequent run on `main` failed or was cancelled, up to and including the current HEAD:

| Run | Date | SHA | Conclusion |
|-----|------|-----|-----------|
| #151 | 2026-08-25T04:23Z | `60fac9e` (current HEAD) | failure |
| #149 | 2026-08-24T22:43Z | `03e9dfe` | failure |
| #147 | 2026-08-24T18:47Z | `173f8f5` | failure |
| #145 | 2026-08-24T18:38Z | `7b87381` | failure |
| #143 | 2026-08-24T18:19Z | `26e8ea1` | failure |
| ... | ... | ... | failure (unbroken back to #104) |
| #102 | 2026-08-22T16:44Z | `603d066` | **success** (last green) |

**Why it matters:** the branch being released is `60fac9e`, whose CI run failed. Nothing in
the last ~3 days of work has passed the project's own release gate. CI is the only automated
check on the Android modules, since they cannot be built without the Android SDK.

**Fix:** treat green CI on the release commit as a hard gate. Do not ship on a red run.

---

## [CRITICAL] The four most recent runs fail in 3–6 seconds — the suite never executes

**Evidence:** run #151 job `build` (id 97684039483) `started_at 04:23:43Z`,
`completed_at 04:23:48Z` — **5 seconds**, conclusion `failure`, and the job exposes no steps.
Runs #145/#147/#149 show the same 3–4 second shape. Log download returns HTTP 404.

A 5-second job that produces no step records did not compile or test anything — it failed at
the runner/account level (Actions minutes exhausted, spending limit, or runner allocation),
not in the code.

**Why it matters:** this is worse than a normal red build. It means the last four pushes to
`main` — including the current release HEAD — received **zero verification of any kind**. The
3 known test failures below are from run #143, the last run that actually executed; whether
anything has regressed since then is simply unknown.

**Fix:** check the repo/org Actions billing and quota, restore runner capacity, then re-run CI
on the release commit and read the result before shipping.

---

## [HIGH] 3 failing tests in the last run that actually executed — all `DesignDoctrineTest`

**Evidence:** run #143 (sha `26e8ea1`): `1014 tests completed, 3 failed`. All three are the
project's own design-doctrine guardrails; the 1011 functional tests passed.

**1. `userContentWrapsAtLargeFontScales`** — DESIGN §14, `maxLines = 1` on user content
- `ui/profile/ProfileActivityMonth.kt:175, 250, 261, 268` (0 allowed, 4 found)
- `ui/profile/ProfileActivityYear.kt:195, 261, 328, 335` (0 allowed, 4 found)

This one has genuine user impact beyond the lint: `maxLines = 1` silently truncates at large
font scales, so a user with accessibility text sizing loses content. Note the codebase argues
both sides itself — `ProfileActivityMonth.kt:~255` carries a comment explaining why a caption
there deliberately omits `maxLines = 1`, while four other call sites in the same file set it.

**2. `typeComesFromTheScale`** — DESIGN §6, inline `fontSize` instead of a theme style
- `ui/profile/ProfileActivityMonth.kt:249, 267, 282, 290`
- `ui/profile/ProfileActivityYear.kt:259, 334, 349, 357`
- `ui/profile/ProfileSurfaceSections.kt:156, 236, 277, 441`

**3. `onlyLadderAlphas`** — DESIGN §5, alphas outside the sanctioned intensity ladder
- `ui/profile/ProfileSurfaceSections.kt:286` → `0.18`
- `ui/profile/ProfileSurfaceSections.kt:257` → `0.4`

The doctrine text notes 0.6 already measures 4.05:1 and fails AA, so a one-off `0.4` on text
is an accessibility contrast risk, not only a consistency one.

**Fix:** for each violation either correct it, or raise the count in
`app/src/test/resources/design-allowlist.txt` with the `#` comment the harness requires. The
`maxLines = 1` ones should be genuinely reviewed rather than blanket-allowlisted.

---

## [MEDIUM] Compiler warnings worth clearing before release

From the run #143 compile log:

- `app/src/test/java/com/forge/app/ui/DesignDoctrine.kt:579:9` — "Only safe (?.) or non-null
  asserted (!!.) calls are allowed on a nullable receiver of type 'File?'." In the test
  harness itself, i.e. the release gate has a nullable-deref warning in its own code.
- `ui/gym/train/components/SetRow.kt:237:17` — "Condition is always 'true'." A dead branch in
  the set-entry row; either the guard is wrong or intent was lost. Worth a look given SetRow
  is on the set-logging path.
- `ui/onboarding/PlanModeMedia.kt:88:24` — "Condition is always 'true'." Same class of issue.
- Widespread KT-73255 annotation-target warnings across `data/repo/*` and `service/*` — these
  become behavioural changes in a future Kotlin release; harmless now, worth pinning
  intentionally with `-Xannotation-default-target`.
- Deprecated non-auto-mirrored icons (`Icons.Filled.TrendingUp/TrendingDown/Login`) — these
  render incorrectly in RTL locales.

---

## [INFO] Local verification is limited in this container

- No Android SDK present (`ANDROID_HOME` unset), so `:app` and `:wear` cannot be built here.
- The sandbox proxy blocks `dl.google.com` (`maven.google.com` 301s into it), so AGP cannot be
  resolved locally. **This is a sandbox limitation, not a project defect** — `mavenCentral`
  and `plugins.gradle.org` are both reachable (HTTP 200), and CI resolves AGP 9.2.1 fine.
- The pure-JVM `:shared` module (protocol codec, rest-timer core, weight-step table) depends
  only on Maven Central artifacts and was run in a standalone harness; see the result section.

---

## [INFO — GOOD NEWS] `:shared` module tests all pass locally, and defend the right things

Run in a standalone Maven-Central-only Gradle harness (same sources, same dependency versions):
`BUILD SUCCESSFUL`, all tests green. Worth recording because these cover exactly the failure
modes a pre-release scan worries about, and they are already handled:

**`WearCodecTest`** — the phone↔watch protocol is genuinely defended:
- `a newer protocol version is dropped as NewerVersion, not a crash` — version skew (new phone,
  old watch) is handled explicitly. This is the #1 Wear-sync corruption risk and it is covered.
- `corrupt payloads decode to Invalid`
- `wrong-shape payload with a valid stamp decodes to Invalid`
- `unknown extra fields are ignored - additive evolution is compatible`
- `every dto round-trips`

**`RestTimerControllerTest`** — clock manipulation is handled:
- `backwardClockJumpReAnchorsToTrueRemaining`
- `backwardClockJumpStillFinishesAfterResume`
- `pausedTimerDoesNotCountDownWhileClockMoves`
- `remainingNeverGoesNegative`

**`WeightStepsTest`** — `steps match the phone stepper table`, `plate exercises step by half a
plate in every unit`.

Caveat on scope: this proves the shared codec and timer core are sound. It says nothing about
how the app-side code *calls* them — e.g. whether the phone actually handles a `NewerVersion`
result rather than ignoring it. That is on the app side, which cannot be built here.
