# editorial-2026-08 — paper copy of Home + Profile before the surface experiment

Captured **2026-08-15** on branch `design/surface-experiment`, from commit
**`cdd50f280c5d4b0eba2e7d2857c8ea464a10f868`** (`cdd50f2`, "Merge remote-tracking branch
'origin/main'").

These files are a **reference copy only**. They live outside every Gradle source set, nothing
imports them, and they do not compile. They exist so the open-editorial version can be read and
diffed side by side with the experiment without switching branches.

## Restore

```
sh .design-backups/editorial-2026-08/restore.sh
```

That undoes the whole experiment:

| Restored to `cdd50f2` | |
|---|---|
| `ui/overview/`, `ui/profile/` | rsync `--delete`, so the files the experiment ADDED to those packages go too |
| `src/test/screenshots/*.png` | the five Overview goldens (unchanged by this branch — see below) |
| `src/test/java/…/DesignDoctrine.kt` | drops the branch-scoped `SANCTIONED` exclusion |
| `src/test/resources/design-allowlist.txt` | undoes the paydown, so the ratchet reads exactly as before |

| Deleted | |
|---|---|
| `ui/experiment/` | the forked card kit — nothing outside the experiment ever imported it |

Nothing else was touched by the branch, so nothing else is touched by the restore: `ui/common/`,
`ui/theme/` and `.claude/DESIGN.md` · `SETTLED.md` · `FAILURES.md` are all byte-identical to `main`.

## What was copied

### `src/overview/` — the whole `ui/overview` package (10 files)

The screen and its **direct sub-components**:

| File | Why |
|---|---|
| `OverviewScreen.kt` | Home itself. Also defines `HeroCta`, `CoachHomeBlock`, `OnThisDayCard`, `MovementLine`, `TopBarIconButton` inline |
| `components/OverviewComponents.kt` | `WeekDayBox`, `OverviewStat`, `RecentRow` |
| `components/OverviewTiles.kt` | `TrophiesTile` |
| `SummarySheet.kt` | the post-session summary sheet Home opens |
| `state/OverviewUiState.kt` | the state Home renders, incl. `OnThisDayMemory` |
| `OverviewViewModel.kt`, `OverviewUiStateMapper.kt` | what produces that state |

Copied so the restore is complete, though Home does not call them directly:
`components/NavTile.kt`, `components/DayEditComponents.kt`, `HistoryComponents.kt`.

### `src/profile/` — the whole `ui/profile` package (33 files)

The screen and its **direct sub-components**:

| File | Why |
|---|---|
| `ProfileScreen.kt` | Profile itself. Also defines `PhotoViewerDialog` inline |
| `ProfileHeader.kt` | `ProfileHeaderCard` — **the blending cover**, listed untouchable |
| `ProfileBody.kt` | `BodyMetricsSection` |
| `ProfileLedger.kt` | `AllTimeSection`, `LifetimeVolumeGraph`, `StandingSection` |
| `ProfileYearHeatmap.kt` | `YearConsistencySection` |
| `TrophyCaseSection.kt` | `TrophyCaseSection` |
| `RankSection.kt` | `RankSection`, `RankInfoSheet` |
| `ProfileExtras.kt` | `GalleryStrip` — **and `GoalLinesSection`, which Home also renders** |
| `ProfileSkeleton.kt`, `ProgressPhotoImage.kt` | load/photo shapes |
| `AvatarPickerSheet.kt`, `BodyweightLogSheet.kt`, `BodyFatLogSheet.kt` | sheets Profile opens |
| `MirrorTestScreen.kt` | defines `AddPhotoChooser`, which Profile calls |
| `state/ProfileUiState.kt`, `ProfileViewModel.kt` | the state and what produces it |

The rest of the package (Mirror Test internals, gallery, body measurements, camera, rank/trophy
renderers) is copied so the restore is complete, not because Profile calls it directly.

### `goldens/` — 5 PNGs

`overview.png` · `overview-200.png` (200% font scale) · `overview-zero.png` · `overview-amoled.png`
· `overview-mono.png`.

**These are the Profile goldens too.** `RecipeScreenshotTest` pins *archetypes*, not screens, and
per `DESIGN.md` §3 both Home and Profile are the **Overview** archetype — they share
`OverviewRecipe`. There is no separate `profile-*.png`, so the five above are the complete golden
set for both screens under test.

**And they do not change when Home and Profile do.** They are captures of `OverviewRecipe`, a
debug-only template, not of `OverviewScreen` or `ProfileScreen`. The surface experiment left the
recipes alone, so the whole golden suite still passes and these five files are byte-identical to
`cdd50f2`. Recorded here anyway so the pair is complete, but they are **not** a before/after of this
branch — see the "no visual diff" note in the final report.

Verified current before copying: `./gradlew :app:testDebugUnitTest --tests '*RecipeScreenshotTest*'`
passed and left `src/test/screenshots/` unmodified, so these bytes are what HEAD actually renders,
not a stale commit.

## Provenance note

The working tree was **not clean** when this snapshot was taken — unrelated Academy/notifications
work was uncommitted and carried onto this branch by `git checkout -b`. None of it touches the paths
above: `git status` on `ui/overview`, `ui/profile` and `src/test/screenshots` was empty, so every
file here is byte-identical to `cdd50f2`.

## Equivalent git restore

If this directory is ever lost, the same result comes from:

```
git checkout cdd50f2 -- forge-android/app/src/main/java/com/forge/app/ui/overview \
                        forge-android/app/src/main/java/com/forge/app/ui/profile \
                        forge-android/app/src/test/screenshots
git clean -fd forge-android/app/src/main/java/com/forge/app/ui/overview \
              forge-android/app/src/main/java/com/forge/app/ui/profile
```
