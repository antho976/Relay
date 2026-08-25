# 08 — Crash paths, release/build config, security & privacy

Scope: `forge-android/` (`:app`, `:wear`, `:shared`), pre-release scan at versionCode 89 / 0.8.8.3, 2026-08-25.

## Triage summary

Part A (crash hunting) came back **much cleaner than expected**. I grepped and then hand-checked
55 `!!` sites, 276 `.first()/.last()/.single()/.reduce()/.random()` terminals, every `items(key=)`
in the Compose tree, every `Map.getValue`, every `substring`/`toInt()`, every division feeding a
Compose fraction, and the whole nav graph. Essentially all of them are provably guarded, usually
with a comment saying why. Specific things I checked and **dismissed** (so they don't get re-flagged
next pass):

- `it.weightLb!!` (~15 sites in Stats/adapt/PR code) — every one is preceded by
  `.filter { it.weightLb != null }`. `LoggedSet.weightLb` really is null for "BW" input, so this was
  the highest-yield candidate; it's handled.
- `ProgressionAdvisor.kt:387` `.maxOf { it.weightLb!! }` — `bouts` is pre-filtered on
  `b.sets.any { it.weightLb != null && !it.isAssisted }`, so `bouts.last()` is non-empty.
- `SessionHistoryScreen.kt:172` `item(key = "day:${day.label}")` — keys come from
  `items.groupBy { label(...) }`, so they are unique by construction; `historyDayLabel` is injective
  per calendar date (the same-year branch omits the year, the other branch includes it).
- `DaySessionContent.kt:346` `key = { "done-${it.index}" }` — keyed by `withIndex()`, deliberately
  not by exercise id (comment explains a day can hold the same exercise twice).
- `MirrorTestViewer.kt:204` `photos[page]` — `photos` is a frozen snapshot, pager `pageCount` is
  `photos.size`, and delete closes the viewer (`viewer = null`).
- `ExerciseChartSheet.kt:191` `volumes[volumes.size - 2]` — call site guards `volumes.size >= 2`.
- `WarmupEngine.kt:206` `RAMP_FRACTIONS.getValue(count)` — `rampSetCount` returns 0..5, 0 returns early, map has keys 1..5.
- `CoachStand.kt:145` `e1rmBySlot.getValue(...)` — only the `withTrend` partition reaches it.
- `AppIconPicker.kt:149` `byFamily.getValue(family)` — `AppIcon.families` is derived from `entries`.
- `StatsBody.kt:73` `currentE1rm / bw` — caller returns early on `bw <= 0.0`.
- Cardio/goal meter fractions — all fall back to a non-zero constant (WHO 150 min) before dividing,
  and `MeterBar` coerces then gates on `frac > 0f` (which also swallows NaN).
- Nav args: `Routes.gymDay(key)` is only called with keys validated against `Program.dayKeys`
  (widget deep link at `ForgeNavHost.kt:98`, notifications feed at `NotificationFeed.kt:182`);
  program-builder day keys are machine-generated `"day-${uid()}"`, never user text.
- String resources: `values/strings.xml` holds exactly one string and no format args or plurals,
  so the whole "format-arg mismatch / missing plural" class is vacuous here.
- Logging: **zero** `android.util.Log` / `println` / `printStackTrace` calls in `app/src/main`,
  `wear/src/main` or `shared/src/main`. No user data is logged, in any build type.
- Network: no `INTERNET` / `ACCESS_NETWORK_STATE` in any of the four manifests; no okhttp, retrofit,
  HttpURLConnection, `java.net.URL`, Socket, WebView, Firebase, Crashlytics or analytics SDK anywhere
  in source or `libs.versions.toml`. **The "no internet permission" claim holds.**
- Secrets: nothing sensitive is tracked. `.env` was untracked in commit `025bb2f` and no `.env`,
  `.jks`, `keystore.properties`, key or token remains in `git ls-files`. `keystore.properties.example`
  contains only `CHANGE_ME` placeholders and is correctly gitignored alongside `*.jks`.
- Room: the migration chain is complete and unbroken, `MIGRATION_12_13` … `MIGRATION_35_36`, all 24
  registered in `ALL_MIGRATIONS`, schemas 1–36 exported, and destructive fallback is limited to the
  pre-lock versions 1–11 with downgrade-fallback gated to `BuildConfig.DEBUG`. This is the single
  best-defended part of the build config.

What follows is what actually survived triage.

---

## [CRITICAL] Auto Backup uploads progress photos and the full training DB to Google Drive, contradicting the app's own "nothing is sent" claim

**File:** `app/src/main/AndroidManifest.xml:70` (`android:allowBackup="true"`), `app/src/main/res/xml/data_extraction_rules.xml:1-40`, `app/src/main/res/xml/backup_rules.xml:1-16`, `app/src/main/java/com/forge/app/ui/settings/SettingsAboutPage.kt:90`

**What:** `allowBackup="true"` combined with a `data_extraction_rules.xml` that *explicitly* opts
physique photos into cloud backup:

```
<cloud-backup>
    <include domain="database"  path="forge.db" />
    <include domain="sharedpref" path="." />
    <include domain="file" path="datastore/" />
    <include domain="file" path="progress_photos/" />
    <include domain="file" path="avatar.jpg" />
</cloud-backup>
```

Settings → About renders the claim `PrivacyClaim("No servers, analytics or tracking", "Nothing is
collected, because nothing is sent.")`. The first half is true (verified above — there is no INTERNET
permission and no network code). The second half is not: Android Auto Backup ships `forge.db`, all
progress photos, the avatar, every SharedPreference and the whole DataStore to the user's Google
Drive. That is not the app exfiltrating data, but it *is* the user's private data leaving the device,
and the copy tells them it doesn't.

The sharpest version of the problem is the interaction with GYMAP-69: the app ships a biometric
"Photo gallery lock" whose entire purpose is that progress photos are sensitive. Those exact photos
are the ones being synced off-device by default, with no in-app disclosure and no way to opt out
short of turning off backup for the whole app in system settings.

Two secondary defects in the same area:

1. **The two backup configs disagree about what is included.** `data_extraction_rules.xml` (API 31+)
   uses explicit `<include>`s, so files in `filesDir` *root* are excluded by omission.
   `backup_rules.xml` (`fullBackupContent`, API 26–30) is include-everything-minus-four-excludes, so
   on Android 8–11 it *additionally* sweeps up `filesDir/crashes/*.txt` (stack traces, which can carry
   user strings such as a malformed exercise name) and the permanent export artifacts
   `forge_export.json`, `forge_sessions.csv`, `forge_prs.csv`, `forge_bodyweight.csv`,
   `forge_cardio.csv` — i.e. a second, plaintext copy of the entire training history
   (`BackupRepository.kt:139,259,321,345,365,376,404`). Same app, same user, materially different
   privacy posture depending on OS version.
2. `crashes/` is never excluded from either config, and is never pruned by age (only to the 10 most
   recent, `ForgeApp.kt:177-181`).

**Scenario:** A user enables the photo gallery lock, takes physique photos, and reasonably believes
from the About screen that they never leave the phone. They are already in the Google backup set.
When they later restore onto a new phone — or if their Google account is compromised — the photos are
there. On an Android 11 device, so is a plaintext JSON dump of every session they have ever logged.

**Fix:** Pick one and say so in the UI.
- If backup should stay on (it is genuinely useful — a lost phone otherwise loses everything), then
  (a) drop `progress_photos/` and `avatar.jpg` from both `<cloud-backup>` and `<device-transfer>`, or
  gate them behind an explicit opt-in; (b) rewrite `backup_rules.xml` to mirror the API 31+ include
  list instead of relying on include-by-default, and add `<exclude domain="file" path="crashes"/>`
  plus excludes for the five export files; (c) change the About copy to something defensible, e.g.
  "No servers, analytics or tracking. Your data stays on your device and in your own Google backup."
- If the "nothing is sent" claim is the product promise, set `android:allowBackup="false"` and lean on
  the in-app .zip backup that already exists (`BackupRepository`), which the user controls.

---

## [HIGH] No DataStore corruption handler + `runBlocking` on the main thread = unrecoverable crash-on-launch loop; and the restore path writes an unvalidated prefs blob into that file

**File:** `app/src/main/java/com/forge/app/data/prefs/PreferencesDataStore.kt:19`, `app/src/main/java/com/forge/app/MainActivity.kt:278-288`, `app/src/main/java/com/forge/app/ForgeApp.kt:92-95`, `app/src/main/java/com/forge/app/data/repo/BackupRepository.kt:744-746`

**What:** Three things compound.

1. The store is declared bare:
   `val Context.forgePreferences: DataStore<Preferences> by preferencesDataStore(name = "forge_settings")`
   — no `corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() }`.
2. `SettingsRepository` exposes **101** flows off `forgePreferences.data` and **not one** of them has
   a `.catch { }`. `grep -n "catch" SettingsRepository.kt` returns nothing. Android's DataStore
   guidance is explicit that `data` throws `IOException`/`CorruptionException` on a read failure and
   that callers must handle it.
3. `MainActivity.onCreate` consumes five of those flows inside `runBlocking` **on the main thread,
   before `setContent`**:

```kotlin
val (introIconKey, themedIntro) = runBlocking {
    val privacy = settingsRepo.privacyMode.first()
    val lockEnabled = settingsRepo.appLockEnabled.first()
    ...
    settingsRepo.appIcon.first() to settingsRepo.themedLaunchIntro.first()
}
```

Any read failure therefore propagates out of `onCreate` as an uncaught exception on every launch.
There is no recovery path in the app — the user's only option is Clear Data, which destroys the
Room DB and the photos too.

That would be a theoretical concern if the prefs file could only be written by DataStore itself. It
can't: `ForgeApp.applyPendingRestore()` runs *before* DataStore is first opened and swaps a
user-supplied file straight into place —

```kotlin
if (pendingPrefs.exists()) {
    // Must match preferencesDataStore(name = "forge_settings").
    if (swapStagedFile(pendingPrefs, File(filesDir, "datastore/forge_settings.preferences_pb"))) ...
}
```

— and `BackupRepository.restoreFromIncoming` stages that file with **no validation at all**:

```kotlin
val pendingPrefs = File(context.filesDir, "pending_restore_prefs.pb")
if (pendingPrefs.exists()) pendingPrefs.delete()
prefsFile?.copyTo(pendingPrefs, overwrite = true)
```

Compare with the sibling database half of the same restore, which gets `isZip` sniffing, the SQLite
magic-byte check (`isSqlite`), a Forge-schema check (`isForgeDatabase`), a newer-than-current
`user_version` rejection, and a `MIN_RESTORABLE_VERSION` floor. The prefs blob gets a `copyTo`. Any
ZIP whose `settings.preferences_pb` entry is not a valid Preferences protobuf — a hand-built zip, a
zip assembled by a third-party tool, a future format change, a file restored from a different app —
sails through, is swapped in at the next boot, and bricks the app.

**Scenario:** User restores a backup (or an Auto Backup restore lands a `.preferences_pb` written by a
newer build). `ForgeApp.applyPendingRestore` swaps it in at boot. `MainActivity.onCreate` calls
`privacyMode.first()`, DataStore throws `CorruptionException`, the process dies. Every subsequent
launch repeats it — the bad file is now the live file and nothing ever deletes it. The user's history
is intact in `forge.db` but permanently unreachable.

**Fix:** All three layers.
- `preferencesDataStore(name = "forge_settings", corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() })`.
- Add `.catch { if (it is IOException) emit(emptyPreferences()) else throw it }` to the shared
  `forgePreferences.data` accessor and route all 101 flows through it (introduce one
  `private val prefs = context.forgePreferences.data.catch { … }` in `SettingsRepository` and map
  from that, rather than 101 edits).
- Wrap the `onCreate` `runBlocking` in `runCatching { }` with sane defaults, and validate the staged
  prefs blob in `restoreFromIncoming` — parsing it with `PreferencesSerializer` in a `runCatching`
  and dropping it on failure is a few lines and matches the care already given to the DB half.

---

## [HIGH] The app lock never re-arms after the screen turns off

**File:** `app/src/main/java/com/forge/app/MainActivity.kt:133-136,162-170`, `app/src/main/java/com/forge/app/security/AppLockManager.kt:86-98`

**What:** The re-lock timer is only started from `onStop`, behind a `userLeaving` gate:

```kotlin
override fun onStop() {
    super.onStop()
    if (isChangingConfigurations || !userLeaving) return
    appLock.onGenuineBackground()
    ...
}
override fun onUserLeaveHint() { super.onUserLeaveHint(); userLeaving = true }
```

`onUserLeaveHint()` is documented as firing only when the activity goes to the background **as the
result of a user choice** — Home, Recents. Android does **not** call it when the screen turns off
(power button or display timeout), on an incoming call, or when another app is brought forward by a
notification tap. In all of those cases `onStop` returns at the guard, `backgroundedAtElapsed` stays
`-1`, and `AppLockManager.onForeground()` sees `since < 0` and leaves `sessionValid = true`.

The gate is therefore only re-armed on a cold process start and on Home/Recents. The default
`appLockTimeoutSec` is `0` ("lock immediately"), which makes the gap starker: the user has explicitly
asked for immediate re-locking and screen-off doesn't do it.

The `userLeaving` gate exists for a real reason — it keeps the launcher-icon alias swap out of the
"a picker we launched is covering us" case, which tears down the task on some OEMs. The bug is that
one flag is being used for two unrelated decisions.

**Scenario:** User enables "App lock", opens Avex, presses the power button and pockets the phone.
Someone else picks it up, gets past the phone's own lock screen (shoulder-surfed PIN, a shared
device, an unlocked-at-home phone), and taps Avex from Recents. Avex opens straight to the training
data — the biometric gate never appears, because the app believes it was never backgrounded.

**Fix:** Decouple the two decisions. Drive the lock from a `DefaultLifecycleObserver` on
`ProcessLifecycleOwner` (`onStop` → `appLock.onGenuineBackground()`, `onStart` → `onForeground()`),
or at minimum call `appLock.onGenuineBackground()` from `MainActivity.onStop()` unconditionally
(before the `isChangingConfigurations || !userLeaving` guard) and leave that guard governing only
`appIconManager.reconcileTo(...)`. Returning from a self-launched picker is already handled correctly
by the timeout, and a 0-second timeout re-locking after a photo-picker round-trip is arguably the
behaviour the user asked for anyway.

---

## [HIGH] `wear/proguard-rules.pro` is empty while the wear module minifies — the phone's enum keep rule is not mirrored on the other end of the wire protocol

**File:** `forge-android/wear/proguard-rules.pro` (0 bytes), `forge-android/wear/build.gradle.kts:50-54`, `forge-android/app/proguard-rules.pro:1-8,21-27`, `forge-android/shared/src/main/kotlin/com/forge/shared/protocol/WearDtos.kt:39,120-124`, `forge-android/shared/build.gradle.kts:19-21`

**What:** `:wear` sets `isMinifyEnabled = true` and points at a `proguard-rules.pro` that contains
nothing. Everything the watch APK keeps comes from `proguard-android-optimize.txt` plus library
consumer rules. Meanwhile `:app`'s rules file opens with the note that enum-constant preservation is
"PERSISTENCE-CRITICAL" and keeps `<fields>` on every enum:

```
-keepclassmembers enum * {
    <fields>;
    public static **[] values();
    public static ** valueOf(java.lang.String);
}
```

The default AGP file keeps only `values()`/`valueOf()`, not `<fields>`.

That asymmetry sits directly on a wire protocol. Two enums cross the Data Layer serialized by
kotlinx.serialization as their constant names:

- `SessionLiveDto.unit: ProtocolWeightUnit` and `ConfigDto.unit` (phone → watch)
- `TimerCommand.Action { SKIP, ADD_30, START }` (watch → phone)

Both APKs are minified independently by two separate R8 invocations. Whatever R8 chooses to do with
enum constant names, doing it on one side of a protocol and not the other is a release-only
interoperability hazard that no debug build and no unit test can observe — `WearCodec.decode`
swallows the failure as `DecodeResult.Invalid` and drops the payload silently, so the symptom is
"the watch's timer buttons stopped working in the Play build", with no crash and no log.

Related and worth correcting while you're in the file: the header comment in
`app/proguard-rules.pro` states —

> Forge is fully offline (no INTERNET permission) and ships **NO reflection-based serialization
> library** — JSON/CSV exports are built by hand.

The first clause is true; the second is not. `:app` declares `api(project(":shared"))` and `:shared`
declares `api(libs.kotlinx.serialization.json)` (1.7.3), so kotlinx.serialization *is* on `:app`'s
runtime classpath and its `@Serializable` classes go through `:app`'s R8 pass. It currently works
because the kotlinx-serialization artifacts carry embedded R8 rules, but the comment will mislead
whoever next decides which keep rules are needed.

**Scenario:** Ship 0.8.8.3 to Play. Phone and watch pair. The user taps "+30s" on the wrist; the
phone decodes `TimerCommand`, R8 renamed `ADD_30` on the watch to something the phone's serializer
does not recognise, `decodeFromString` throws, `WearCodec` returns `Invalid`, the command is dropped.
Nothing is logged. QA on debug builds sees none of it.

**Fix:**
- Copy `:app`'s enum block and `-keepattributes SourceFile,LineNumberTable` /
  `-renamesourcefileattribute SourceFile` into `wear/proguard-rules.pro` (the second pair also makes
  watch crash traces readable, which they currently are not).
- Belt and braces on the protocol itself: put an explicit `@SerialName` on every constant of
  `ProtocolWeightUnit` and `TimerCommand.Action` so the wire string is a source-level literal that R8
  cannot touch on either side.
- Fix the stale comment in `app/proguard-rules.pro`.
- Add a `:shared` unit test that asserts the exact encoded JSON of one `TimerCommand` and one
  `ConfigDto` (a golden string), so a serial-name change becomes a red test rather than a field report.

---

## [HIGH] `targetSdk = 35` days before Play's API-36 deadline

**File:** `forge-android/app/build.gradle.kts:44`, `forge-android/wear/build.gradle.kts:31`

**What:** Both modules ship `targetSdk = 35` while `compileSdk` is already `36`. Google Play's annual
target-API rule requires new apps and updates to target the API level released roughly a year prior,
enforced at the end of August each year; the 2026 cut moves that to API 36 (Android 16). Today is
2026-08-25.

I am flagging this to be **verified against current Play Console policy** rather than asserting the
date from memory — but the exposure is asymmetric: if the deadline is what I believe, an upload after
it is rejected outright, and if it isn't, bumping is nearly free here because `compileSdk` is already
36 and the module comment says target was deliberately left at 35 only to avoid changing runtime
behaviour when compile was bumped for Health Connect.

**Scenario:** Release build is cut on 0.8.8.3, upload lands on or after the cut-off, Play Console
refuses the bundle. The fix then has to be made and re-tested under release pressure rather than now.

**Fix:** Bump both modules to `targetSdk = 36` and run a device pass over the API-36 behaviour changes
that actually touch this app — the foreground-service (`specialUse`) rules, predictive back (already
opted in via `enableOnBackInvokedCallback="true"`), and edge-to-edge enforcement, all of which this
app already handles. Confirm the exact deadline in Play Console before cutting the release either way.

---

## [MEDIUM] FLAG_SECURE ignores the gallery lock, so photo-lock-only users are still visible in Recents and screenshots

**File:** `app/src/main/java/com/forge/app/MainActivity.kt:284,293`

**What:** The window flag is derived from two of the three privacy settings:

```kotlin
applyPrivacyMode(privacy || lockEnabled)
...
combine(settingsRepo.privacyMode, settingsRepo.appLockEnabled) { privacy, lock -> privacy || lock }
    .collect { secure -> applyPrivacyMode(secure) }
```

`settingsRepo.galleryLockEnabled` is a fully independent preference with its own toggle
(`SettingsSecurityPage.kt:73-78`, "Photo gallery lock — Require an unlock to view your progress
photos") and it is not in that expression. The comment two lines above even states the intended
rule — "turning on a lock implies keeping the app out of the recents preview / screenshots, as every
app-lock feature does" — which the code then only applies to the app lock.

**Scenario:** A user who wants Avex itself open but their physique photos protected turns on only
"Photo gallery lock". They unlock the gallery with their fingerprint, browse, and press Home. The
Recents thumbnail shows the photo grid. A screenshot, a screen recording, or any accessibility
service with screen-capture works normally on the unlocked gallery. The lock protected the door and
left the window open.

**Fix:** Include the third flow:
`combine(privacyMode, appLockEnabled, galleryLockEnabled) { p, a, g -> p || a || g }`, and mirror it
in the synchronous first-frame call at line 284.

---

## [MEDIUM] CI never verifies the release artifact beyond compiling it, and never runs the Room migration test

**File:** `.github/workflows/ci.yml:52-88`, `forge-android/app/src/androidTest/java/com/forge/app/data/db/MigrationTest.kt`

**What:** The pipeline runs, in order: `testDebugUnitTest` + `:shared:test`,
`:app:verifyRoborazziDebug`, `assembleDebug`, `assembleRelease`. That is a genuinely good suite for a
solo project — 102 unit-test files and screenshot goldens at two font scales is more than most apps
ship with. The gaps are specifically in the release direction:

1. **No `connectedAndroidTest`.** `MigrationTest.kt` is the *only* androidTest in the repo and the only
   automated guard on the 12→36 migration chain that `DatabaseModule` deliberately refuses to
   destructively fall back from. It never runs in CI. The one test protecting against "release build
   crashes on open for every existing user" is manual-only.
2. **No `lint` / `lintVitalRelease`.** `lintVitalRelease` is the check that catches exactly the class
   of manifest and resource defects in this report — `FullBackupContent` inconsistencies, exported
   components without permissions, `allowBackup` warnings.
3. **`assembleRelease` compiles R8 but nothing ever installs or launches the minified APK.** Every
   R8-only defect in this document (the empty wear rules, the enum/serial-name asymmetry) is invisible
   to a pipeline that only checks that R8 *finished*.
4. Minor: the workflow comment says "There is no committed Gradle wrapper", but the wrapper *is*
   committed at `forge-android/gradle/wrapper/gradle-wrapper.jar` and pins 9.4.1 — the same version
   CI provisions by hand. Switching to `./gradlew` removes the chance of the two drifting.

**Scenario:** A future schema bump lands with a missing or wrong migration. Unit tests are green,
screenshots are green, `assembleRelease` is green, the APK ships, and every user with existing history
crashes on launch while a fresh install works fine — which is exactly the failure mode the schema lock
was designed to prevent, with the one test that would catch it never running.

**Fix:** Add a `lintVitalRelease` step (it is fast and would have flagged several items here), add an
emulator job running `connectedDebugAndroidTest` so `MigrationTest` actually executes, and — highest
value for the least work — add a job that installs the minified release APK on an emulator and
launches it once, asserting it reaches the first frame.

---

## [MEDIUM] `WearSyncService` is exported with no permission

**File:** `app/src/main/AndroidManifest.xml:214-234`

**What:**

```xml
<service
    android:name=".service.wear.WearSyncService"
    android:exported="true">
    <intent-filter>
        <action android:name="com.google.android.gms.wearable.MESSAGE_RECEIVED" />
        <data android:scheme="wear" android:host="*" android:pathPrefix="/cmd" />
    </intent-filter>
    ...
```

No `android:permission`. The documented hardening for a `WearableListenerService` is
`android:permission="com.google.android.gms.permission.BIND_WEARABLE_LISTENER"`, which constrains the
binder to Google Play services. Without it the component is reachable by any app on the device.

What sits behind it is not read-only: `onMessageReceived` dispatches `/cmd/log-set`,
`/cmd/undo-set`, `/cmd/set-rpe` and `/hr` straight into `WearCommandHandler` → `SetLogUseCase` → Room
writes (`WearSyncService.kt:50-53`). `/cmd/undo-set` deletes the user's most recent set. The service
is also declared to wake the process when the app is dead, so this works with Avex closed.

The impact is bounded — a local app, no network, and the commands are `commandId`-deduped and
session-validated — so this is data integrity rather than data theft. But it is a free hardening on a
component that writes to the user's training history.

**Scenario:** Any other installed app binds the exported service and replays crafted `/cmd/undo-set`
messages, silently deleting sets from the user's active session. The user sees history quietly
disappear with no explanation and no log.

**Fix:** Add `android:permission="com.google.android.gms.permission.BIND_WEARABLE_LISTENER"` to the
`<service>` element. Verify wear sync still works on a paired device afterwards — this is the
supported configuration, but it is worth one round-trip on hardware.

---

## [MEDIUM] The app-lock overlay leaves the nav host in the accessibility tree

**File:** `app/src/main/java/com/forge/app/MainActivity.kt:359-379`

**What:** The gate is a sibling in a `Box`, drawn over a still-composed `ForgeNavHost`:

```kotlin
Box(Modifier.fillMaxSize()) {
    when (onboardingDone) { true -> { ForgeNavHost(initialDayKey = pendingWidgetDayKey) } ... }
    if (onboardingDone == true) {
        val locked by appLock.appLocked.collectAsState()
        if (locked) { AppLockScreen(...) }
    }
    ...
}
```

`AppLockScreen` paints an opaque gradient and swallows taps with a no-op `clickable`
(`AppLockScreen.kt:106-110`), but it does not `clearAndSetSemantics {}`. The nav host beneath remains
in the semantics tree, so TalkBack — or any accessibility service, which is a permission a user can be
socially engineered into granting — can enumerate and read the screen behind the lock. FLAG_SECURE
blocks pixels; it does not block semantics.

Note the gallery gate does not have this problem: at `ForgeNavHost.kt:400-412` it *replaces*
`MirrorTestScreen` rather than covering it, which is the correct shape.

**Scenario:** Phone is locked by Avex's app lock. TalkBack is on (a real configuration for a
low-vision user, not just an attacker's tool). Swiping through the "locked" screen reads out the
overview beneath it — session names, volumes, bodyweight.

**Fix:** Wrap the gate in `Box(Modifier.fillMaxSize().semantics { isTraversalGroup = true })` and add
`.clearAndSetSemantics { }` to the nav-host branch while `locked` is true, or simplest: mirror the
gallery gate's shape and don't compose `ForgeNavHost` at all while `appLocked` is true. The nav host's
state survives in the back stack either way.

---

## [LOW] Two accent-hex parsers, one of which throws, used on a value that round-trips through DataStore and Room

**File:** `app/src/main/java/com/forge/app/ui/theme/ColorExt.kt:10-17`, `app/src/main/java/com/forge/app/ui/common/AccentHex.kt:7-8`, `app/src/main/java/com/forge/app/ui/gym/train/components/DayCard.kt:66`, `app/src/main/java/com/forge/app/ui/gym/train/components/DayCardComponents.kt:47`

**What:** The codebase has both a throwing and a non-throwing hex parser.

```kotlin
fun String.toAccentColor(): Color {
    val hex = removePrefix("#")
    val value = hex.toLong(16)          // throws NumberFormatException BEFORE the length check
    return when (hex.length) { 6 -> …; 8 -> …; else -> error("Bad hex color: …") }
}
```
versus `fun parseAccentHex(hex: String): Color = runCatching { … }.getOrDefault(Color.Gray)`.

`toAccentColor()` is used on `(item.customAccentHex ?: item.plan.accentHex)` — the day-list cards.
`customAccentHex` comes from DataStore (`SettingsRepository.setDayColor`, an unvalidated
`String` write) and `plan.accentHex` from the Room column `program_day.accent_hex`. Today both are
only ever written from the fixed `DAY_ACCENTS` palette, so this is **not currently reachable** and I
am not claiming it as a live crash. It is on the list because the value crosses two persistence
boundaries and a restore, `ForgeTheme` and `ForgeWidget` both already use the safe
`runCatching`/`parseColor` form for the same kind of value, and `InsightEngine.kt:276` already
constructs a `DayPlan(accentHex = "")` — safe only because that object is never rendered. The
docstring's justification ("these are static program constants") stopped being true when day colours
became user-settable.

**Scenario:** A restored backup, a hand-edited DB, or a future code path that renders the
`InsightEngine` synthetic `DayPlan` puts a blank or malformed hex in front of `DayCard`, and the
entire gym Train tab throws on composition.

**Fix:** Delete `toAccentColor()` and route the two call sites through `parseAccentHex()`. One parser,
non-throwing, matching what `ForgeTheme` and `ForgeWidget` already do.

---

## [LOW] Five sequential DataStore reads inside `runBlocking` on the main thread at every cold start

**File:** `app/src/main/java/com/forge/app/MainActivity.kt:278-288`

**What:** `onCreate` blocks the main thread on `privacyMode.first()`, `appLockEnabled.first()`,
`amoledMode.first()`, `appIcon.first()` and `themedLaunchIntro.first()` before `setContent`. The
reason is sound and documented — FLAG_SECURE and the lock state must be correct on the very first
frame, and the window background must match the theme so the post-splash frame doesn't flash. But it
is main-thread disk I/O on the launch path, and it is precisely what this app's own debug StrictMode
policy (`ForgeApp.kt:186-190`, `.detectDiskReads()`) is configured to report.

**Scenario:** Cold start on a device with slow or contended storage (low-end phone, a restore in
progress, a heavily fragmented filesystem). The first read pays the full file-open cost on the main
thread. Worst case it crosses the ANR threshold; typical case it just makes launch slower than it
needs to be.

**Fix:** Only the first `.first()` pays the file read — the rest hit DataStore's in-memory cache — so
the cheap win is to collapse the five into one `runBlocking { forgePreferences.data.first() }` and
read all five keys off that single `Preferences` snapshot. Combine with the `runCatching` wrapper from
the DataStore finding above.

---

## [LOW] The FileProvider root is the whole of `filesDir`, which contains the photos, the DataStore and the crash logs

**File:** `app/src/main/res/xml/file_paths.xml:1-6`, `app/src/main/AndroidManifest.xml:236-245`

**What:** `<files-path name="forge_exports" path="." />` roots the provider at `filesDir` itself, so
its addressable surface includes `progress_photos/`, `datastore/forge_settings.preferences_pb`,
`avatar.jpg` and `crashes/` — not just the export artifacts the name suggests. The provider is
`exported="false"` with `grantUriPermissions="true"`, so only URIs the app explicitly grants are ever
reachable and there is no current path that grants one of those. This is defence in depth, not a live
hole.

**Scenario:** A future share/export feature builds a content URI from a caller-influenced filename and
hands it to the share sheet; because the provider root is `filesDir`, a traversal or a mistaken path
resolves to a photo or the preferences file instead of an export.

**Fix:** Move the exports (`forge_export.json`, `forge_sessions.csv`, `forge_prs.csv`,
`forge_bodyweight.csv`, `forge_cardio.csv`, the weekly export, the session exports and the PDFs) into
`filesDir/exports/` and narrow the mapping to `<files-path name="forge_exports" path="exports/" />`.
That also fixes the API ≤30 Auto Backup over-inclusion noted in the CRITICAL finding, since the export
files would then sit under a path you can exclude cleanly.
