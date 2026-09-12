# Plan creation and rest review

Reviewed 2026-09-11 in `relay/silver-ibis`. Scope: generated split structure, equipment and experience filters, goal rep ranges, set allocation, weekly scheduling, and the phone rest timer and its shared prescription policy.

## Changes

| Finding | Correction |
| --- | --- |
| Ordinary 8–12-rep compounds started at 210 seconds: a 180-second base plus an overly broad heavy-rep bonus. | Base is 120 seconds for compounds and 90 seconds for isolation. Only compound prescriptions whose entire numeric range is six reps or fewer add 60 seconds. Timed and per-side text cannot trigger this bonus. |
| Only a pre-existing “brutal” exercise rating changed rest. Ratings entered after logging did not update the running timer. | Easy sets subtract 30 seconds, hard sets add 15, brutal sets add 30. RPE takes precedence over a set tag and then the exercise rating. Phone ratings adjust the current open rest without discarding elapsed time, pause state or manual additions. Explicit exercise overrides still take precedence. |
| Completion-to-completion logging intervals could lengthen rest even though they include another working set and equipment setup. | Habit tuning can shorten a prescription but cannot lengthen it. Reported effort can justify longer recovery. |
| The same lift appearing on multiple days could resolve another day's rep prescription through the global lookup. | Phone rest calculations retain the current slot's displayed rep range while resolving the performed movement's classification. |
| Two full-body sessions were placed on consecutive default weekdays; three days used once-weekly push/pull/legs. | Two days default to Monday/Thursday. Three days use full-body A/B/C on Monday/Wednesday/Friday. Four and five days also have distributed default rest slots. |
| Seven days ended in arms immediately before the next push day. | Seven days use push/legs/pull twice, followed by a short core/calves session. This is an accessory session, not another full upper-body workout. |
| The next-workout resolver did not account for yesterday's muscle exposure. | A shared recovery guard requires an intervening calendar day before suggesting overlapping work. It includes substantial press/pull/leg assistance, checks weekly wraparound, and feeds the directive, day list, statistics, widget and reminder. Completion dates govern recovery, including sessions spanning midnight. |
| Strength slots could prescribe 4–6 reps to isolation exercises. Weight-loss goals automatically became high-rep prescriptions. | Isolation strength-goal slots use 8–12 or 12–15 reps. Weight-loss goals retain normal resistance-training ranges. Timed, per-side and AMRAP prescriptions retain their units. |
| Defaults began at four heavy sets and three accessory sets, and direct arm/delt budgets ignored compound assistance. | Start from three compound/hypertrophy sets and two pump sets. Direct arm, delt and glute ceilings are lower to allow for compound work. Session allocation is capped at 24 sets, with lower beginner/deload budgets. Advanced volume scales above intermediate where ceilings permit. |
| Learned personal ceilings could be exceeded by a structural minimum, and priority muscles could exceed those personal limits. | Personal ceilings take precedence, including omitting zero-set slots. Priority headroom applies only to population defaults. |
| Pins, difficulty fallbacks and empty-day fallbacks could undo exercise restrictions. | Known problem-area exclusions, equipment, dislikes and the experience ceiling are applied before selection. Empty slots are not filled with an unrelated muscle or a forbidden movement. |

## Coaching evidence and interpretation

The [2026 ACSM resistance-training guidance](https://acsm.org/resistance-training-guidelines-update-2026/) supports regular training of major muscle groups, heavier loads for strength, and sufficient weekly volume for hypertrophy. Its approximate ten-set hypertrophy benchmark is a starting point, not a required minimum for every person or every small muscle. This informed the three-day full-body structure and moderate initial volume.

The [2024 rest-interval systematic review](https://pubmed.ncbi.nlm.nih.gov/39205815/) found a small hypertrophy advantage above 60 seconds and no appreciable difference beyond 90 seconds in its pooled analysis. That supports practical 90–120-second defaults; it does not establish a universal two-minute ceiling for difficult strength work.

The [2024 proximity-to-failure meta-regressions](https://pubmed.ncbi.nlm.nih.gov/38970765/) support using effort to interpret training stimulus, with uncertainty about an exact optimal RIR. The app's easy/hard/brutal timer increments and 24-set session ceiling are conservative product choices, not thresholds proven uniquely optimal by those studies.

The requested intervening rest day is implemented as a conservative scheduling policy. It is not a claim that every possible consecutive-day protocol is ineffective or unsafe.

## Validation

- `CoachingPolicyTest`: 420 generated weeks across seven day counts, three equipment setups and 20 seeds. Checks adjacent-day muscle exposure, including Sunday/Monday.
- Profile checks cover all four goals and beginner/intermediate/advanced volume, plus priority muscles, low personal caps, restrictions and impossible preferences.
- `RestEffortPolicyTest`: default durations, effort precedence, explicit overrides, heavy-rep boundaries, timed/per-side ranges and protection against inflated habit tuning.
- Existing generator, volume, goal, rest, schedule, app and shared regression suites are included in the final Gradle run.
- Final run: **1,691 app tests and 30 shared tests passed**, with zero failures or skipped tests. `:app:lintDebug` and `git diff --check` passed.
- Command: `./forge-android/gradlew -p forge-android :app:testDebugUnitTest :shared:test :app:lintDebug --offline --no-parallel --max-workers=1`, with `ANDROID_HOME` pointing to the installed SDK.
- An earlier parallel lint analysis crashed internally in Kotlin PSI. The final serial analysis completed successfully; no lint rule was disabled.

## Limits and rollout behavior

These are defensible defaults, not proof of a uniquely optimal or mistake-free prescription for an individual. Load availability, technique, actual proximity to failure, recovery and adherence still determine whether the dose fits.

The recovery guard uses tracked session completion and the current plan's movement metadata. It cannot infer unlogged training, and it does not reconstruct every historical custom swap or skipped set. Manual workout choice remains available. Current muscle metadata also groups some anatomically distinct regions together.

Contradictory restrictions can leave a slot or day empty. The preview remains safe to render, and no forbidden movement is inserted to hide the missing coverage. A dedicated explanation and resolution flow for such impossible combinations remains outside this change.

Generated structure, sets and reps apply on generation; existing stored plans are not migrated. Explicit rest preferences remain authoritative. No app installation, physical-device timer interaction or watch interaction was performed. The watch uses the shared base prescription; the live rating-to-timer adjustment added here is on the phone.
