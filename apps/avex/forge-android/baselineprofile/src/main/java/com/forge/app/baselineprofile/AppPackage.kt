package com.forge.app.baselineprofile

/**
 * The package these benchmarks launch — the app's `applicationId`, not its Kotlin namespace.
 *
 * Both files here named `com.forge.app`, which is the namespace: the value that decides where the
 * generated `R` and `BuildConfig` classes live. The installed app is `com.quietsoftware.avex`
 * (`app/build.gradle.kts`), so `startActivityAndWait()` was reaching for a package that is not on
 * the device. Nothing in the pipeline noticed, because neither of these runs in CI — so the profile
 * `app/src/main/baseline-prof.txt` was never generated, and profileinstaller has been shipping
 * nothing while `implementation(libs.androidx.profileinstaller)` and a `baselineProfile(...)`
 * dependency in the app made it look otherwise.
 *
 * One constant, in one file, so the two cannot drift again — and so the next person editing it sees
 * why it is not simply the namespace. The instrumented smoke test in `.github/scripts/instrumented.sh`
 * carries the same warning for the same reason.
 */
internal const val APP_PACKAGE = "com.quietsoftware.avex"
