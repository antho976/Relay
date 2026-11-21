# Avex Android

Avex is a fully offline gym tracker and adaptive training coach built with Kotlin and Jetpack
Compose. The Gradle project contains the phone app, Wear OS companion, shared protocol/domain code,
and baseline-profile module.

## Modules

- `:app`: phone app, Room database, Health Connect, training, coach, Academy, Profile, exports
- `:wear`: Wear OS set logging, live heart rate, tiles, complications, and phone pairing
- `:shared`: pure Kotlin protocol, timer, and weight-step code used by phone and watch
- `:baselineprofile`: Macrobenchmark and baseline-profile generation

The shipping application ID is `com.quietsoftware.avex`. Internal `com.forge.*` packages, database
names, settings keys, notification IDs, and legacy import names remain compatibility identifiers.

## Local setup

Open `forge-android/` in Android Studio or use the wrapper from this directory:

```fish
set -lx ANDROID_HOME /home/anthony/Android/Sdk
set -lx ANDROID_SDK_ROOT /home/anthony/Android/Sdk
./gradlew :app:assembleDebug :wear:assembleDebug --no-daemon
```

Install the phone and watch debug APKs with matching `.debug` application IDs so the Wear Data
Layer can pair them.

## Verification

```fish
./gradlew :app:testDebugUnitTest :shared:test :wear:testDebugUnitTest \
  :app:verifyRoborazziDebug :app:lintRelease :wear:lintRelease --no-daemon
```

The doctrine and screenshot tests cover Compose rules plus 100% and 200% font-scale recipes. Device
checks still matter for migrations, Health Connect, notifications, the widget, biometrics, camera,
and phone-watch behavior.

## Release signing

Copy `keystore.properties.example` to the gitignored `keystore.properties` and reference the
registered Play upload key. With a complete file, both release modules use that signing config.
Without it, local and CI release builds remain unsigned and are not uploadable.

Build signed bundles with:

```fish
./gradlew :app:bundleRelease :wear:bundleRelease --no-daemon
```

The full Play checklist lives in [`.claude/RELEASING.md`](../.claude/RELEASING.md).

## Architecture conventions

- Compose UI reads values from `ui/theme/` and follows `.claude/DESIGN.md`.
- KSP is used for Room and Hilt.
- Room migrations and exported schemas are versioned in `app/schemas/`.
- Avex has no analytics, crash-reporting SDK, cloud sync, or `INTERNET` permission.
- Charts use Compose `Canvas`; no chart dependency is included.
