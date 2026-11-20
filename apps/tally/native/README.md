# Tally RS

Tally in Rust, drawn with wgpu (asked for 2026-11-19: the owner writes their apps in Rust now).
It installs beside the Kotlin app as **Tally RS** (`com.quietsoftware.tally.rs`, violet icon) and
takes over the real package once it can do everything the Kotlin app does.

## What it is made of

| Piece | What |
|---|---|
| `relay-money` (git dependency on antho976/RelayV4) | The money rules and the SQLite ledger: the Rust port of `core/`, held to the same tests, which Relay's engine already runs. Tally RS adds no money rules of its own. |
| `crates/tally-rs/src/gfx.rs`, `shader.wgsl` | One wgpu pipeline: every shape is an instance of one quad with a signed-distance edge (Feel the Beat's renderer, plus clip regions). |
| `draw.rs` | The canvas: boxes, soft shadows, arcs, text (fontdue, Geist and Sora), icons (zeno), pictures. Units are 1/390 of the phone's width, like the design canvas. |
| `kit.rs`, `theme.rs`, `pictures.rs` | The premium look (DESIGN.md): Light and Dark palettes, cards, picture cards, rows, badges, meters, keys, carousels, and the pictures. |
| `overview.rs`, `entry.rs` | The screens ported so far. |
| `glyphs.rs`, `category_icons.rs` | Generated: Relay's glyphs from the Kotlin `Glyphs.kt`/`Icon.kt` (`tools/gen_glyphs.py`), and the category icons as the Kotlin app draws them, Material Rounded from Google's Apache-2.0 SVGs (`tools/gen_category_icons.py`). |
| `desktop.rs` | A phone-shaped window (winit) and `--shot` screenshots with no window. |
| `android.rs`, `android/` | GameActivity (the AndroidX activity that gives a native app the soft keyboard) and a Gradle module that only packs `libtally.so` with it. No Kotlin or Java of our own. |

## Building

```
cd native
cargo test                                          # the app's rules
cargo run                                           # the desktop window (~/.local/share/tally-rs)
cargo run -- --shot out.png --screen overview       # overview | entry | entry-typed | empty, --scroll N
tools/build-lib.sh                                  # libtally.so for arm64 into android/src/main/jniLibs
cd .. && ./gradlew :rs:assembleDebug                # native/android/build/outputs/apk/debug/rs-debug.apk
```

The Arch rust package has no Android standard library, so `build-lib.sh` uses the official
Rust 1.99.1 in `~/.local/share/rust-android` (rustc, cargo, and rust-std for x86_64 Linux,
aarch64-linux-android and x86_64-linux-android, from static.rust-lang.org, checksums verified). It
does not touch the PATH. `tools/build-lib.sh x86_64` builds for the `Tally_API36` emulator.

## Where the port stands

0.3 (2026-11-19): the premium look (DESIGN.md, "Premium, dark and light"; PORTING.md, "The
premium look"). Light, Dark and System palettes; studio pictures in `assets/pictures` (a light and
a dark version of each, decoded off the UI thread by `pictures.rs`); tabs Overview, Entries, +,
Insights, Search; a drawer you swipe right to open (`app.rs`); icon E, fanned cards, whose light or
dark version the launcher shows to match the app (two activity-aliases switched by
`TallyActivity.setIconDark`). 91 tests.

0.2 (2026-11-19): every screen of the Kotlin app is ported, each module owning its page(s):
`overview`, `entry` (new, edit, delete with Undo, Usual chips), `activity` (Entries, one
category's or account's entries, Search), `insights` (Month, Trend, Calendar), `plan` (Budgets,
Bills, Goals and their editors), `accounts`, `invest`, `categories`, `settings` (Appearance,
Currency and dates, Backup and export, About), `import` (bank statements, Wealthsimple, Tally CSV),
`banks` (Plaid Hosted Link; docs/PLAID.md), `sync` and `onboarding`. 82 tests. Checked on the
`Tally_API36` emulator: onboarding, every tab, Back, a goal, Settings, Night, an entry saved.

Sync (`sync*.rs`): pairing as the Kotlin app does (`/api/tally`, `/enroll`, `/claim`, the door),
then two-way `money.sync`. Tally RS never asks to connect by itself (each ask DMs the owner), and
can never replace the server's ledger: its first sync only pulls (`sync_ledger.rs` makes a
replace unrepresentable; tests check it against a fake server).

Writes relay-money has no call for (bills, goals, contributions, categories, account kind and
opening balance, the week start) go through `Store::sql()` in `*_store.rs`, each tested by reading
back through relay-money. They belong in relay-money eventually.

Android: GameActivity plus one Java class, `android/src/main/java/com/tally/rs/TallyActivity.java`,
for what Android gives only to Java: the file picker's result, the share sheet's FileProvider,
Back (AndroidX would finish the activity), and the system bars' icon colour.

Not done yet:

1. Background work while the app is closed: sync (WorkManager in the Kotlin app), bill reminder
   notifications, the weekly backup. Bills that fell due are posted when the app opens.
2. The server's Plaid ops (docs/PLAID.md) are not in RelayV4 yet; Connected banks says so.
3. Insights' Worth lens, the repeat toggle on a new entry, Kotlin's onboarding set-up questions.
4. Then it takes the package `com.quietsoftware.tally2` and the Kotlin app retires.
