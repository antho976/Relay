# Releasing Avex to Google Play

Avex ships as a phone app with a Wear OS companion under `com.quietsoftware.avex`. Work through
this list from an updated `main`. A compiled unsigned bundle is not a releasable artifact.

## 1. Version and notes

- [ ] Set the intended `versionName` in both `app/build.gradle.kts` and `wear/build.gradle.kts`.
- [ ] Increase the phone `versionCode`; keep Wear at phone code plus 100,000 unless Play history
      requires a different unused code.
- [ ] Add the release to `ui/settings/Changelog.kt`, `.claude/CHANGELOG.md`, and the matching Play
      notes under `docs/`.
- [ ] Confirm phone `targetSdk` meets the current Play deadline.

## 2. Local release gate

```fish
cd forge-android
set -lx ANDROID_HOME /home/anthony/Android/Sdk
set -lx ANDROID_SDK_ROOT /home/anthony/Android/Sdk
./gradlew :app:testDebugUnitTest :shared:test :wear:testDebugUnitTest \
  :app:verifyRoborazziDebug :app:lintRelease :wear:lintRelease --no-daemon
```

- [ ] All tests pass.
- [ ] Roborazzi comparisons pass after reviewing any intended pixel changes at 100% and 200% font.
- [ ] Phone and Wear release lint report zero errors.
- [ ] GitHub Actions is green on the exact commit being released.

## 3. Upload signing

- [ ] Copy `keystore.properties.example` to `keystore.properties` and point it at the registered
      Play upload key. Never generate a replacement key when an upload key already exists.
- [ ] Keep the keystore and passwords outside git and in a second recoverable backup.
- [ ] Build both signed bundles:

```fish
./gradlew :app:bundleRelease :wear:bundleRelease --no-daemon
```

- [ ] Confirm both tasks report a release signing config and verify the artifacts before upload:

```fish
jarsigner -verify -verbose -certs app/build/outputs/bundle/release/app-release.aab
jarsigner -verify -verbose -certs wear/build/outputs/bundle/release/wear-release.aab
```

## 4. Privacy and Play declarations

- [ ] Publish `.claude/PRIVACY.md` at the privacy URL used by the Play listing.
- [ ] Confirm both Health Connect privacy actions open the same in-app policy.
- [ ] Make Play Data Safety and Health Apps declarations match the permissions in the final manifest.
- [ ] Confirm the policy, listing, and app agree on Health Connect, camera, biometrics,
      notifications, foreground services, Wear sensors, sharing, retention, and deletion.

## 5. Upgrade and device validation

- [ ] Upgrade the exact current Play build without uninstalling and confirm retained data migrates.
- [ ] Run migration instrumentation tests, including the oldest supported schema.
- [ ] Smoke-test phone API 26 and a current Android version.
- [ ] Smoke-test Wear API 30 and a current Wear version.
- [ ] Log and finish a workout, create a PR, run the rest timer, receive notifications, and update
      the widget.
- [ ] Export, restore, and import both Avex-named and legacy Forge-named files.
- [ ] Verify Health Connect rationale, reads, writes, revocation, and no-grant behavior.
- [ ] Verify pairing, heart-rate capture, tiles, complications, and disconnected watch behavior.
- [ ] Check normal and 200% phone font scale, enlarged Wear text, dark theme, and AMOLED.

Room uses destructive fallback for schemas 1 through 11. Do not release until Play install history
proves those versions are no longer supported, or replace that fallback with a tested migration path.

## 6. Play upload

- [ ] Upload the signed phone and Wear AABs to the intended track.
- [ ] Add the matching release notes and review the device catalog exclusions.
- [ ] Refresh phone, watch, and large-font screenshots when the visible product changed.
- [ ] Complete Play's automated checks and resolve every blocking warning before rollout.
- [ ] Install the Play-delivered build from the test track and repeat the critical smoke path.
