# Forge — doctrine decision log

Satellite of `.claude/DESIGN.md`. Why the doctrine changed, dated, newest first.

`SETTLED.md` records what was removed from the *product*. This records what changed about the
*rules*, so a future reader can tell a deliberate reversal from an accident. §16's protocol requires
an entry whenever a rule is added, changed or retired.

---

## 2026-07-25 — §6 gains a mono anchor rung: equal size is not equal presence

Row labels moved from 11 → 13 to stop being the smallest thing on a page whose job is naming metrics
— which put them on the section anchor's own rung. A section anchor is usually a SHORT word with wide
tracking ("BODY") sitting over long rows ("BODY FAT"): at a matched 13sp both measured an identical
26px cap height on device, the anchor carried less visual mass, and the header read as the *smaller*
of the two.

So `EditorialHeader` now takes `MonoSectionAnchor` (15sp) — one step above the rows beneath it. The
general rule §6 keeps: **when two labels must rank, rank them by SIZE**; tracking and colour cannot
carry a hierarchy on their own.

## 2026-07-24 — Fourth audit pass: reviewing the templates as design, not as regex

Much quieter round: one real defect, one hardening, and three angles that came back clean.

**1. `LiveRecipe`'s stepper carried dead code.** A `Spacer(Modifier.height(0.dp))` inside a *Row* —
where a height does nothing at all — while the actual spacing came from a leading space inside the
string (`" $unit"`). Both wrong, and worse in a file whose entire job is to be copied: a template
teaching a no-op and a spacing hack. Replaced with a real `Spacer(Modifier.width(4.dp))`.

**2. Sub-references were unguarded.** `§4.10` resolves only while §4 has ten numbered items; reorder
that list and every sub-reference silently points at nothing, which the parent-section check cannot
see because §4 itself still exists. Added `everySubReferenceResolves`, proved by referencing §4.99.

**Came back clean.** The comment-stripper survives eight edge cases (a `//` inside a string, an
escaped quote before one, URLs, raw-string bodies, and violations hidden in comments — which are
correctly ignored rather than flagged). All existing sub-references resolve. The parity tests fail
loudly, not silently, when the doc format is mangled.

**Left as a judgment call, not changed:** in the Live stepper the `−` and `+` sit at the screen
edges with the figure left-aligned between them. Defensible, and common in other apps, but §3's Live
archetype prioritises reach over padding and the `+` is a long way from a thumb. Flagged rather than
redesigned, because that is a taste call and taste calls are Antho's.

---

## 2026-07-24 — Third audit pass: the gates were agreeing with themselves

**1. A line break disabled three rules.** The scanner matched line by line, so `title = {` with
`Text(` wrapped onto the next line read as compliant, as did a split `maxLines = 1` and a wrapped
`tween(`. No such splits exist today, but a redesign that reformats a long call would introduce them
silently. Those three rules now match against the whole comment-stripped file with a line-number map;
counts were unchanged, which is the evidence that no false positives came with it. Verified by
injecting a wrapped `title = { Text("Settings") }` and watching it get caught at the right line.

**2. The screenshot goldens could not see a truncation bug.** Adding `maxLines = 1` plus an ellipsis
to a list row — the exact §14 violation — changed **zero** goldens, because every fixture rendered
short strings like "Pull B". The visual gate was agreeing with itself. This was the doctrine's own
rule broken in its own test fixtures: §12 says design against the longest realistic string, not the
demo one. Added long-content goldens (realistic exercise names at 100% and 200%); the same bug now
fails `list-long-200`. 16 goldens -> 20.

**3. The §9 duration check passed on a coincidence.** It looked for each duration's number anywhere
in the section, and deleting "Draw 900" from the table still passed because an unrelated 900 (the
ForgeSwitch spring) sat a few lines below. Now matches the labelled pair.

**Probed and sound:** the other parity tests fail loudly when the doc format is mangled — renaming a
§6 voice marker, rewording the §7 radii line and deleting a §14 table row each fired the right test
rather than silently matching nothing.

---

## 2026-07-24 — Second audit pass: false negatives, and doctrine filed as debt

**1. `.clickable { }` was invisible to the accessibility rule.** The regex required a parenthesis,
but Kotlin's trailing-lambda form has none — **24 unlabelled tap targets** never reached the gate.
Widened to `\.clickable\s*[({]`; debt rose 906 -> 930, all of it previously hidden.

**2. Thirteen violations were the doctrine's own implementation.** `ripple` in `BounceClick.kt` is
§9's TalkBack fallback; `SnackbarHostState` in `SnackbarControllerHost.kt` is §8's one Undo snackbar;
the literal palette in `ConfettiOverlay.kt` is the one deliberately polychrome moment. Counting the
shared implementation of a rule as a violation of it made the debt figure lie in the *other*
direction, and no amount of cleanup could ever retire those entries. Moved into a `SANCTIONED` map in
the scanner, with a test that fails if one of those paths goes stale — otherwise a rename would
silently un-exempt the file and the fix would look like "add it back to the allowlist".

**3. Robolectric fetches its `android-all` jars at test time into `~/.m2`,** outside Gradle's
dependency resolution, so `setup-gradle`'s cache never covered them. Every CI run would have
re-downloaded a few hundred megabytes and inherited Maven Central's bad days as flaky tests. Cached
explicitly in `ci.yml`.

**Checked and clean:** raw strings hold only shaders, SQL and the MIT licence, so the scanner's
inability to see inside multi-line raw strings costs nothing today; `Card (` with a space does not
occur; `AppIconManager` only ever toggles its own `AppIcon.entries`, so the new debug launcher icon
cannot collide with it; PNG goldens need no `.gitattributes`; and the screenshot gate genuinely
verifies when run after the unit-test step rather than being skipped as up-to-date — checked by
breaking a golden and watching five fail in CI order.

---

## 2026-07-24 — Audit pass: four more defects before trusting any of it

Deliberately hunting rather than re-running. Everything below was wrong and is now fixed.

**1. Four packages of user-facing copy were never scanned.** The copy scope covered `ui`, `domain`,
`service` and `data`. It missed `program`, which holds every exercise description rendered in the
swap picker — **93 em dashes** sitting there unseen — plus `widget` (home-screen copy), `security`
and `appicon`. Added all four. Frozen debt went 810 -> 906, which is the net getting wider, not the
code getting worse.

**2. `alpha = 0f` and `alpha = 1f` were false positives.** The regex strips a trailing `f`, so it
captured "0" and "1", neither of which was on the ladder. Fully transparent and fully opaque are not
ladder choices; both are now allowed.

**3. The recipe gallery was unreachable.** It previewed in the IDE but nothing referenced it, and the
nav host lives in `main` and must not know debug code exists — so "browse the archetypes on a real
device" was simply false. Now a debug-only `RecipeGalleryActivity` with its own launcher icon,
verified present in the debug manifest and absent from release.

**4. Goldens had zero pixel tolerance.** Font rasterisation differs between machines and JDKs, so
goldens recorded on a dev box tend to fail the first time CI renders them for reasons unrelated to
design. Added a 0.1% threshold, then checked it is not too loose: a 4dp spacer change still fails
five goldens.

**Also verified this pass:** a genuinely cold build (build directories deleted, build cache and
configuration cache disabled) runs 668 tests green and reproduces every golden; `:wear` still builds
with the Roborazzi plugin applied to `:app`; nothing new is caught by `.gitignore`; multi-line raw
strings (101 of them, mostly AGSL shaders) do not produce false positives.

---

## 2026-07-24 — Redesign-readiness: per-token allowlist keys, and a safe paydown mode

**Why.** Pressure-testing the gate against the workflow a UI revamp actually puts it through, rather
than just re-running it.

**A third allowlist design, because the second still had a hole.** Per-FILE counts meant a fix and a
new violation in the same file cancelled out: cleaning up three alphas in `OverviewTiles.kt` while
introducing a fourth netted to a decrease, so the new one was invisible to the gate *and* to the
paydown check. Keys are now **(rule, file, token)** with exact counts — a new value is a new key, and
more of an existing value raises its count. 386 entries, same 810 total. Verified by reproducing the
exact case that slipped through; it now fails both checks.

**`-Dforge.paydown=true`.** Rewriting screens removes old violations, which the gate reports as
"debt was paid down, lower these numbers". Correct, but a slog across thirty files, and the tempting
escape was a full `-Dforge.regen=true` that would silently swallow any new violation from the same
pass. Paydown lowers counts and **refuses to raise any**, failing with the offender named and
writing nothing. That keeps the ratchet honest while a redesign is in flight.

**Gotcha worth knowing:** a system property alone does not invalidate a Gradle test task, so the
maintenance commands need `--rerun-tasks` or they no-op and report success. Documented in `SKILL.md`.

**And the worst one, found last.** The doctrine tests read `.claude/` from disk, which Gradle cannot
infer from the classpath, so the test task stayed UP-TO-DATE whenever only documentation changed —
meaning `DoctrineParityTest` and `DoctrineSelfCheckTest`, the two suites whose entire job is catching
doc drift, **did not run when the doc drifted.** A stale 99-line `SKILL.md` sat past its 80-line cap
through several green builds. Fixed by declaring `.claude/` as a task input; verified by editing only
`DESIGN.md` and watching the suite fire. The loader cap was then raised 80 → 110 deliberately, since
what pushed it over was the redesign workflow (process, which belongs there) rather than restated
rules (which do not).

---

## 2026-07-24 — Screenshot goldens (Roborazzi), and the first bug they caught

**Why.** Everything else in the repo checks source text. Clipping, overlap and spacing regressions
live in rendered pixels, and §14's "must survive 200%" was a promise nothing could verify.

**It worked, against expectations.** Paparazzi's stable line (1.3.5) couples tightly to AGP
internals and this project runs AGP 9.2.1 / Gradle 9.4.1 / Kotlin 2.2.10, so the spike used
**Roborazzi 1.70.0 + Robolectric 4.14.1**, which sit on the unit-test side. It resolved and ran
first try. Full record takes about two minutes cold, ten seconds warm.

**16 goldens**, committed under `app/src/test/screenshots/`: the six archetype recipes at 100% and
200% font scale, plus Overview at zero data, on AMOLED, and in monochrome. CI runs
`:app:verifyRoborazziDebug` and uploads diffs on failure. Verified it actually detects a regression
by changing one `Spacer` from 2dp to 18dp — four goldens failed.

**What it found immediately.** `SettingsPrimaryAction` and `SettingsOutlineAction` had no horizontal
gutter and no width bound, so the capsule sized to its label's intrinsic width. Fine at 100%; at 200%
"Update Health Connect" ran off **both** edges of the screen. Shipping code, invisible to every
static rule here, and invisible to the eye at normal font scale. Fixed by bounding each capsule in a
`Row(fillMaxWidth + 24dp gutter)`.

It also made the accent-contrast problem concrete: `Manage permissions →` at 200% is visibly dim
against the ground, which is the 2.35:1 measured in `SETTLED.md` rather than a rendering artifact.

**Re-record deliberately.** `:app:recordRoborazziDebug` overwrites the goldens. A changed golden is
a question, not a chore: look at the diff before accepting it.

---

## 2026-07-24 — Enforcement round 2: widened scan, 9 new rules, doc↔code parity

**Why.** The gate written earlier that day had a hole in exactly the place it mattered most: §11's
copy rules are written for *generated* lines (coach, milestones, recaps, notifications), but the scan
only covered `ui/`, and generated copy lives in `domain/` and `service/`. Five violations were
shipping, two of them in notifications.

**Changed.**

- **Scan roots are now per-scope.** `ui/` takes every rule; `domain/`, `service/` and `data/` take
  copy rules only; `:wear` takes copy + colour. Layout rules deliberately do not run outside `ui/` —
  a gate that cries wolf gets ignored, and clamped text is normal on a round watch face.
- **`:wear` is gated at all for the first time.** It was hardcoding the phone palette with nothing
  connecting the two, while `WEAR.md` said they "may not drift".
- **Nine new rules.** `hype` and `paren-plural` (§11); `m3-card` (§1), `spinner` (§12), `rtl` (§14),
  `toast` (§12), `snackbar-host` (§8), `ripple` (§9), `unlabelled-clickable` (§14).
  The first three had **zero** occurrences — they cost nothing and exist purely so the doctrine's
  most load-bearing bans can never quietly regress.
- **`Icons.Filled` was considered and rejected as a rule.** §8 explicitly permits stock icons for
  top-bar chrome and forbids them in content, and a static scan cannot tell the two apart. A rule
  with 51 unavoidable false positives would have taught everyone to ignore the gate.
- **Five §11 violations rewritten** rather than allowlisted: the two notification exclamation marks,
  the "the work is showing" praise ungrounded in data, and three em dashes in coach/insight lines.
  `PrMilestoneTest` was asserting the old string and was updated with it.
- **`DoctrineParityTest`** — every value the doc states must equal the value in code: §5 colours,
  §6 type scale, §7 radii, §9 durations, §8's component inventory (both directions), §14's contrast
  table recomputed from the real palette, and wear/phone palette parity.
- **`DoctrineSelfCheckTest`** — the line cap, every `§N` reference resolving, every satellite and
  recipe existing, the loader staying a router, and the wordmark being spelled "Avex".

**Bugs this round caught in the previous round's work.**

1. *The allowlist key was too coarse.* Keys were `rule + file + token`, and generic tokens like
   "em dash in string" meant allowlisting one em dash in a file exempted every future one. Found by
   injecting a violation and watching the em-dash rule stay silent. Now **exact per-file counts**, so
   any addition *or* fix moves a number and paydown shows in the diff as "12 -> 11".
2. *The `:wear` root silently did not exist.* `File(".").absoluteFile` keeps its trailing `/.`, so
   `parentFile` returned the module rather than the Gradle root; the root was filtered out and the
   whole watch module went unscanned while every test passed. Now `canonicalFile`, plus two tests
   that assert every expected root resolves and contains sources. **A gate that scans nothing passes
   everything.**

**Line cap raised 400 → 420.** The §8 inventory is now machine-verified complete in both directions,
which added names that cannot be trimmed without making the parity test fail. Raising it deliberately,
in the same commit, is the process §16 already described; the alternative was letting the file exceed
a number it claimed to respect.

**Deferred.** Remediating the 831 frozen violations (see `AUDIT.md` for the ranked order). A
`traversalIndex`/focus-order rule was considered and left out: phone-only and portrait-only, so the
default order is almost always right, and a wrong rule here is worse than none.

---

## 2026-07-24 — The split: one doctrine file became a core plus satellites

**Why.** `DESIGN.md` had reached 299 lines / 50KB, 2.5× the ~200 lines its own loader claimed, and
was loaded in full before every UI task including one-line tweaks. 30% of it was the per-screen
inventory (§2) — the part that rots fastest and helps least when deciding *how* to build. It was also
almost pure prohibition: ~190 "never"s and zero worked examples, which can correct a design but
cannot generate one.

**Changed.**

- **Split** into a core (rules) plus `MAP.md` (inventory), `SETTLED.md` (removals), `WEAR.md`,
  and later `FAILURES.md` and `AUDIT.md`. Verified byte-identical: reconstructing the original from
  the pieces diffed clean against the pre-split file.
- **Added what prohibition cannot supply**: six decision ladders (§2), the seven-state matrix (§12),
  a physics chapter (§14 — font scale, measured contrast, touch, Canvas semantics), a one-clause
  *why* on each principle, and per-archetype checklists.
- **Six compiling archetype recipes** in `src/debug`, each showing the section rhythm, where the mark
  goes, the zero-state branch inline, and a 200% font preview. Debug-only, so they never ship, but
  they compile against the real primitives and break if one drifts.
- **`FAILURES.md`** — twelve named failure modes with symptom, cause, fix. Naming a failure is the
  point: "this is a button wall" is a diagnosis; "too many buttons" is an opinion.
- **The loader became a pure router.** It had drifted into saying `• Forge` instead of `• Avex`,
  teaching "Baseline set" as a good verdict when §11 bans that exact string, and carrying three
  off-by-one section references. It now restates nothing.

**Contrast measured, two failures found and NOT silently fixed.** Accent-coloured `action →` links
run 2.35–3.40:1 and the inline error line 3.69:1, both below AA. Both are mandated by the doctrine
itself, and fixing either changes the app's look — so they are recorded in `SETTLED.md` as open
decisions with options, and §14 forbids *new* accent- or error-coloured body text until resolved.

**A headline number was corrected.** An initial grep counted 570 fixed `.height()` calls and framed
this as a large migration. Classifying them showed 517 are `Spacer`/`Canvas`/drawn marks where a
fixed height is correct, and of 7 flagged, 5 were false positives. Real exposure: ~2 sites. The
actual 200% risk is clamped content and the type-scale bypass, not fixed heights.
