# Releasing & Sharing Forge

A practical pre-flight checklist for cutting a build and sharing it with someone else.
Forge ships **free, offline, and sideloaded** — there is no Play Store track or billing —
so "release" here means "a signed APK a stranger can install and use on their own data."

Work top-to-bottom. Anything still open lives in `.claude/roadmap-2026-06-18.md`
(Cat 12 Release, Cat 10 De-personalization, Cat 14 Code Health).

---

## 1. Version & changelog
- [ ] Bump `versionCode` + `versionName` in the app `build.gradle`.
- [ ] Add the release entry to `CHANGELOG.md` **first** (newest at top): a plain-English
      "What's new (user-facing)" bullet or two, plus a short "Developer history" note.
- [ ] Commit as `Version x.y.z` (matches the existing tag convention).

## 2. Build is green
- [ ] Full JVM unit suite passes (`testDebugUnitTest`).
- [ ] Debug build compiles.
- [ ] Release build compiles **and** passes `lintVitalRelease` — CI's `assembleRelease`
      runs it, and release-only lint errors (e.g. `FullBackupContent`) fail the build
      even when tests and the debug build are green.
- [ ] R8 / minify is on and the **obfuscated release APK** has been smoke-tested
      (not just the debug APK).

## 3. Signing & identity
- [ ] Release keystore exists and is referenced by a `signingConfig` (not the debug key).
- [ ] Keystore + passwords are stored somewhere safe and **out of git**.
- [ ] `applicationId` is no longer `com.forge.app` placeholder if shipping to others
      (Cat 10 P0) — or consciously accept it for a private share.
- [ ] Real launcher icon in place (no placeholder) (Cat 12 P0).

## 4. Multi-user / de-personalization
- [ ] No first-person / owner-specific copy in the seeded program or onboarding
      ("Antho" refs, the frozen MWM-989 pool) — a new user starts clean.
- [ ] A brand-new user opening the app sees welcome/empty states, not a wall of zeros.
- [ ] Onboarding produces a sensible generated program from the user's own inputs.

## 5. Data safety
- [ ] Room schema is migration-locked (no destructive fallback); migrations run clean.
- [ ] Whole-DB backup **and** restore both work on-device.
- [ ] Confirm the schema export + migration test run clean in CI.

## 6. On-device smoke test (do this every time)
- [ ] One structured end-to-end session on a real device (Cat 14 P0): log a workout,
      hit a PR, finish, check Stats / rank / coach update, then back up and restore.
- [ ] Notifications behave (training reminder + weekly recap respect their opt-in/hour).
- [ ] No crash on cold start, on first-run (no data), and after a restore.

## 7. Distribution
- [ ] Produce the artifact: signed **APK** for sideload, and/or an **AAB** variant.
- [ ] Clean sideload path that doesn't collide with the debug `applicationId` suffix.
- [ ] (Optional) GitHub Actions release workflow builds + attaches the signed APK.
- [ ] Tell the recipient how to enable "install unknown apps" for their installer.

## 8. Hand-off notes for the recipient
- [ ] Everything is offline and on-device; their data never leaves the phone.
- [ ] Backup/restore is the "moving to a new phone" path — point them at it in Settings.
- [ ] Where to find: the coach, Stats, units toggle, and the gestures guide (Settings).

---

_Keep this list honest: if a box isn't actually ticked, say so in the share message
rather than implying it's done._
