# Forge — Android (native)

Native Kotlin/Compose rewrite of the React/Expo prototype that lives in `../forge/`.
The prototype is the **feature spec**, not the build target — see [`../forge/src/WorkoutTracker.web.jsx`](../forge/src/WorkoutTracker.web.jsx).

## Status — Phase 0

The skeleton builds and installs. Every Nav destination resolves to a placeholder screen tagged "PHASE 0". Nothing persists yet; nothing computes yet.

What is wired up:

- Gradle (Kotlin DSL) + version catalog at [`gradle/libs.versions.toml`](gradle/libs.versions.toml)
- Compose (Material 3 dark theme), Activity, single-Activity nav
- Hilt DI (Application + Activity entry points, ClockModule, DatabaseModule)
- Room (minimal `AppMeta` schema, KSP wired, schema export at `app/schemas/`)
- DataStore Preferences dependency (not yet used)
- Coroutines + Lifecycle Compose integration
- 5 placeholder screens routed through `ForgeNavHost`

## First-time setup

1. Open `forge-android/` in Android Studio (Iguana or newer).
2. Let it run "Sync Project with Gradle Files" — first sync downloads ~1 GB of dependencies and creates the Gradle wrapper jar (~10 min).
3. Build & Run on an emulator or your phone (USB-debug + Run).

The Compose-side `ForgeTheme` governs the in-app look. The XML `Theme.Forge` only exists to set the window background so there's no white flash on cold launch.

## Layout

```
app/src/main/java/com/forge/app/
├── ForgeApp.kt           # @HiltAndroidApp
├── MainActivity.kt       # @AndroidEntryPoint, hosts ForgeNavHost
├── core/time/            # Injectable Clock
├── data/db/              # Room: ForgeDatabase + entities + dao
├── di/                   # Hilt modules (Database, Clock)
└── ui/
    ├── theme/            # Color, Type, Shape, ForgeTheme
    ├── nav/              # Routes, ForgeNavHost
    ├── common/           # PlaceholderScreen (gone by end of Phase 3)
    ├── welcome/  overview/  cardio/  trophies/
    └── gym/train/        # DayListScreen, DayScreen
```

## Phase plan

| # | Phase | Outcome |
|---|---|---|
| **0** ✅ | Scaffold | App builds, installs, navigates between placeholder screens |
| 1 | Static program data | `program/` package with PROGRAM, SWAPS, TROPHIES, tutorials |
| 2 | Room schema + DAOs + repositories | DB persists sessions, sets, customizations, cardio, moods, trophies |
| 3 | **Gym training feature** | Log a real workout end-to-end on the phone |
| 4 | Overview screen | Real weekly stats |
| 5 | Stats subtab | Heatmap, strength curves, weekly volume, PR timeline |
| 6 | Trophies | Catalog + unlock evaluator + animation |
| 7 | Cardio | Manual entry, recent list, weekly stats |
| 8 | Polish | Haptics on PR, background notification for rest timer, export, mood prompt |
| 9 | Nutrition | Planned separately when we get there |

## Conventions

- One Compose screen per file. Screens >~300 lines → split sub-components into a sibling `components/` package.
- One ViewModel per screen. ViewModels >~150 lines → extract use-case logic into `domain/`.
- One Room entity per file in `data/db/entities/`; one DAO per file in `data/db/dao/`.
- KSP, not KAPT (faster builds, Kotlin-native).
- No multi-Gradle-module split (intentional — single `:app` module).
- No chart library (custom Compose `Canvas`).
- No analytics, no crash reporting, no cloud sync.
