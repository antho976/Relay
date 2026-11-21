# Avex: `apps/avex`

Avex is Antho's gym app for Android and Wear OS (Kotlin, Compose, Room), published on Google Play
as `com.quietsoftware.avex`. It lived in `antho976/Avex` until 2026-11-20 and moved here with its
history; old pull request numbers in commit messages read `antho976/Avex#N`. It is its own app:
nothing in Cargo or `apps/relay-android` builds it, and it builds nothing of theirs.

## Building and verifying

The Gradle project is `forge-android/`. **Run Gradle from there**, never from the repository root,
and never `./gradlew` from `apps/relay-android`: both projects have an `:app` module.

```fish
cd apps/avex/forge-android
./gradlew :app:testDebugUnitTest :shared:test :wear:testDebugUnitTest \
  :app:verifyRoborazziDebug :app:lintRelease :wear:lintRelease
```

CI is `.github/workflows/avex.yml` at the repository root (it runs only for changes under
`apps/avex/`); its helper scripts stay in `apps/avex/.github/scripts/` and run from `apps/avex`.
Releases are tagged `avex-v<versionName>` (`avex-release.yml`), not `v*`. The release checklist is
`.claude/RELEASING.md`; design rules are `.claude/DESIGN.md` and `.claude/design/`, and the
doctrine tests read them, so a doc edit there can turn the build red.

## The promise that is easy to break

The public build has **no `INTERNET` permission**, and the privacy policy (`.claude/PRIVACY.md`; the
published page is the separate `antho976/avex-privacy` repository) says Avex "physically cannot upload anything". Only the
`personal` build type (`app/src/personal/`) may talk to Relay. Never add `INTERNET`, a network
library or an upload path to `main` or `release`, even when a Relay task is about the cloud copy
or `gym.upload`; CI fails a release APK that holds it.

## What Relay reads from Avex

`crates/relay-gym` reads Avex's training-history export (`BackupRepository.exportFullDataJson`,
`exportVersion` 1) and the `personal` build uploads it (docs/GYM.md, docs/CLOUD.md). A change to
that export's shape changes both sides in the same piece of work: the exporter here and
`relay_gym::export::read` with its tests.
