# Margin

An offline spending and budget tracker for Android. You log what you spend in two seconds, and the
app answers one question at all times: how much room is left this month, and is the month on pace.

Kotlin, Jetpack Compose and Material 3, Room over SQLite, Hilt. No account, no servers, no
internet permission. Visual language inherited from Avex.

## Install a test build

Every CI run on this branch uploads an artifact called **margin-apks** (Actions tab, open the run,
scroll to Artifacts). It holds:

- `app-release.apk`: the minified build, signed with a throwaway debug key. Use this one to judge
  speed and feel.
- `app-debug.apk`: installs beside it as "Margin" with the `.debug` id, with LeakCanary watching
  for leaks.

Unzip, copy to the phone, open it (allow installing from your file manager once). A tagged release
(`v*`) also publishes the APK on the Releases page.

To try the app with something in it: on the first screen choose **try it with sample data**, or
later go to Settings, Backup & data, Load sample data. Sample accounts are named "Sample ..." so
they can never pass for real data; Erase everything removes them.

## What it does

- **Home**: money in and out this month, the margin drawn as the budget meter with today's pace
  tick, what each remaining day can take, the envelopes furthest over pace, recent entries,
  upcoming bills, account balances.
- **Activity**: every entry by day, month by month, with search across all time and filters.
- **Add entry** (the + button, from every tab): keypad, category, account, date, note with
  suggestions, transfers between accounts, optional repeat.
- **Plan**: the monthly budget and per-category budgets with pace, bills and subscriptions that
  post themselves on their due date, savings goals with contributions.
- **Insights**: the month line (drag to scrub any day against the pace), where the money went,
  largest expenses, six-month trend, a calendar heat grid.
- **Settings**: accent colour and pure black, currency, payday budget month, week start,
  accounts, categories, JSON backup and restore, CSV export and import, sample data, erase.

## Build

```
./gradlew :core:test                 # the money arithmetic, plain JVM, seconds
./gradlew :app:testDebugUnitTest     # Room, repositories, doctrine, screenshots (Robolectric)
./gradlew :app:verifyRoborazziDebug  # the same, comparing screenshot goldens
./gradlew :app:assembleDebug
```

Needs JDK 21 and the Android SDK with `platforms;android-37.0`.

## CI

`.github/workflows/ci.yml`, on every push:

| Job | What it proves |
|---|---|
| Guard | wrapper checksum, no keystores or build output committed, no INTERNET permission |
| Core | money formatting, budget periods, pace, recurrence, CSV and backup codecs, sample data, voice rules |
| Verify | Room DAO and repository tests on SQLite, recurring posting, backup round trip, design doctrine, theme contrast, screenshot goldens at 100% and 200% font, Android Lint, R8 release, merged manifest has no INTERNET, APK size budget |
| Instrumented | the real UI flow on an emulator with LeakCanary failing any test that leaks, device SQLite, and a cold launch of the minified APK |

`record-screenshots.yml` re-records the goldens and the Room schema on demand; `release.yml`
turns a `v*` tag into a GitHub Release (signed with the real key when the `MARGIN_*` secrets are
set, otherwise a debug-signed pre-release).
