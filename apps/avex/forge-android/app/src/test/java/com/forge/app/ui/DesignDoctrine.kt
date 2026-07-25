package com.forge.app.ui

import java.io.File

/**
 * The mechanically checkable half of `.claude/DESIGN.md`.
 *
 * Prose can state a rule; only a test can keep it true. Before this existed the doctrine claimed
 * "the only allowed alphas" while 42 distinct alpha values lived in `ui/` — a rule violated 445
 * times is not a rule, and once one stated absolute is visibly false every other one quietly
 * downgrades to advisory in the reader's mind. Everything here is a rule a regex can settle, so the
 * doc can spend its words on the judgment a regex cannot.
 *
 * Every rule enforces an opinion the doctrine already holds. Nothing here is imported from another
 * design system.
 *
 * ## Scopes
 *
 * Rules are grouped so each can run where it makes sense. The first version scanned only `ui/`,
 * which meant §11's copy rules — written explicitly for *generated* lines (coach, milestones,
 * recaps, notifications) — never saw `domain/` or `service/`, where that copy actually lives. Five
 * violations were shipping as a result, two of them in notifications.
 *
 * ## The allowlist
 *
 * Existing violations are frozen in `src/test/resources/design-allowlist.txt` as EXACT counts per
 * (rule, file, token), so this went green on day one. Any new violation changes a number, and so
 * does any fix, which means debt paydown shows in the diff instead of vanishing.
 *
 * During a redesign, bank cleanups with `-Dforge.paydown=true`: it lowers counts and REFUSES to
 * raise any, so it cannot hide a violation introduced in the same pass.
 */
object DesignDoctrine {

    /** DESIGN §5 — the intensity ladder. `muted 0.65` is the floor (measured 4.54:1 on Pearl). */
    // "0" and "1" are here because the regex strips a trailing `f`: `alpha = 0f` (fully
    // transparent, e.g. a gradient's far stop) and `alpha = 1f` (fully opaque) are not ladder
    // choices at all, and flagging them was a false positive.
    private val ALLOWED_ALPHA =
        setOf("0", "1", "0.15", "0.25", "0.35", "0.6", "0.65", "0.7", "1.0", "1f", "0f")

    /**
     * Total frozen debt (2026-07-24): 906 across ui 709 · program 96 · data 40 · domain 38 ·
     * wear 17 · service 3 · widget 2 · security 1.
     *
     * It moved 831 -> 810 as the recipes were cleaned and §5's two named exceptions (placeholder,
     * gradient/scrim) moved out of the allowlist into the scanner where they belong, then 810 -> 906
     * when `program`, `widget` and `security` were added to the copy scope. That last jump is not a
     * regression: `program` holds every exercise description rendered in the swap picker, and 93 em
     * dashes had been sitting there unseen the whole time. Debt going UP because the net got wider
     * is the system working.
     *
     * Three rules — `m3-card`, `spinner`, `rtl` — carry ZERO debt. They cost nothing today and exist
     * so the doctrine's most load-bearing bans can never quietly regress.
     *
     * Lower this as debt is paid down; raising it is a reviewable decision, not a convenience.
     * `design/AUDIT.md` has the per-rule breakdown and the recommended order of paydown.
     */
    const val ALLOWLIST_BASELINE = 917

    data class Violation(val rule: String, val path: String, val token: String, val line: Int) {
        /**
         * Allowlist key: (rule, file, token), counted exactly.
         *
         * Two earlier designs were wrong. A key-set with no counts let one allowlisted em dash in a
         * file exempt every future one. Per-FILE counts fixed that but opened a subtler hole: fixing
         * three alphas while introducing one in the same file nets to a decrease, so the new
         * violation was invisible to the gate AND to the paydown check. Counting per token closes
         * both: a new value is a new key, and more of an existing value raises its count.
         */
        val key: String get() = "$rule $path $token"
        override fun toString() = "$path:$line  [$rule] $token"
    }

    // ── Scopes and roots ───────────────────────────────────────────────────────────────────────

    private enum class Scope {
        /** Layout, controls, motion — only meaningful where Compose UI is written. */
        LAYOUT,
        /** §11 voice rules — meaningful anywhere a user-facing string is authored. */
        COPY,
        /** §5 colour and alpha — meaningful anywhere something is painted. */
        COLOR,
    }

    private data class Root(val dir: File, val label: String, val scopes: Set<Scope>)

    // Gradle runs unit tests with the module directory as CWD, but that is a default rather than a
    // guarantee, so walk up until the tree we are meant to police is actually underneath us.
    // canonicalFile, not absoluteFile: File(".").absoluteFile keeps the trailing "/.", so its
    // parentFile is the module itself rather than the Gradle root — which silently resolved the
    // :wear root to a non-existent path, filtered it out, and scanned nothing while passing green.
    private val moduleRoot: File by lazy {
        generateSequence(File(".").canonicalFile) { it.parentFile }
            .firstOrNull { File(it, "src/main/java/com/forge/app/ui").isDirectory }
            ?: error("Could not locate the :app module from ${File(".").canonicalPath}")
    }

    /** Every root the scan expects to exist. A missing one is a bug, not a reason to skip. */
    private val REQUIRED_ROOT_LABELS =
        setOf("ui", "domain", "service", "data", "wear", "program", "widget", "security", "appicon")

    /** Repository root — the directory holding `.claude/`. Shared with the doctrine parity tests. */
    val repoRoot: File by lazy {
        generateSequence(moduleRoot) { it.parentFile }
            .firstOrNull { File(it, ".claude/DESIGN.md").isFile }
            ?: error("Could not locate .claude/DESIGN.md above ${moduleRoot.canonicalPath}")
    }

    val designDoc: File get() = File(repoRoot, ".claude/DESIGN.md")
    fun satellite(name: String): File = File(repoRoot, ".claude/design/$name")
    fun appSource(relative: String): File = File(moduleRoot, "src/main/java/com/forge/app/$relative")
    fun debugSource(relative: String): File = File(moduleRoot, "src/debug/java/com/forge/app/$relative")
    fun wearSource(relative: String): File =
        File(moduleRoot.parentFile, "wear/src/main/java/com/forge/wear/$relative")

    private val roots: List<Root> by lazy {
        val main = File(moduleRoot, "src/main/java/com/forge/app")
        listOf(
            // Compose UI: everything applies.
            Root(File(main, "ui"), "ui", setOf(Scope.LAYOUT, Scope.COPY, Scope.COLOR)),
            // Recipes are the template every screen is copied from, so they are held to the
            // doctrine like any shipping screen — arguably more strictly.
            Root(
                File(moduleRoot, "src/debug/java/com/forge/app/ui"), "ui",
                setOf(Scope.LAYOUT, Scope.COPY, Scope.COLOR)
            ),
            // Where generated copy actually lives. Layout rules would be meaningless here, and
            // running them would produce noise that gets the whole gate ignored.
            Root(File(main, "domain"), "domain", setOf(Scope.COPY)),
            Root(File(main, "service"), "service", setOf(Scope.COPY)),
            Root(File(main, "data"), "data", setOf(Scope.COPY)),
            // Also user-facing, and missed by the first pass: `program` holds every exercise
            // description shown in the swap picker (93 em dashes were sitting there ungated),
            // `widget` is home-screen copy, `security` and `appicon` carry user-visible strings.
            Root(File(main, "program"), "program", setOf(Scope.COPY)),
            Root(File(main, "widget"), "widget", setOf(Scope.COPY)),
            Root(File(main, "security"), "security", setOf(Scope.COPY)),
            Root(File(main, "appicon"), "appicon", setOf(Scope.COPY)),
            // The watch is a separate Gradle module and was previously ungated entirely, while
            // hardcoding the phone palette. Layout rules do not translate to a round 1.4" screen
            // (clamped text is normal there), so it takes copy + colour only.
            Root(
                File(moduleRoot.parentFile, "wear/src/main/java/com/forge/wear"), "wear",
                setOf(Scope.COPY, Scope.COLOR)
            ),
        )
    }

    /** Labels of roots that actually resolved on disk — asserted against [REQUIRED_ROOT_LABELS]. */
    fun resolvedRootLabels(): Set<String> = roots.filter { it.dir.isDirectory }.map { it.label }.toSet()

    fun requiredRootLabels(): Set<String> = REQUIRED_ROOT_LABELS

    /** Number of source files the scan actually read, per root label. A zero is a bug. */
    fun scannedFileCounts(): Map<String, Int> =
        roots.filter { it.dir.isDirectory }
            .groupBy { it.label }
            .mapValues { (_, rs) -> rs.sumOf { sources(it).size } }

    private fun sources(root: Root): List<File> =
        if (!root.dir.isDirectory) emptyList()
        else root.dir.walkTopDown().filter { it.isFile && it.extension == "kt" }.toList()

    private fun relative(file: File, root: Root): String =
        root.label + "/" + file.relativeTo(root.dir).path.replace(File.separatorChar, '/')

    // ── Reading source without comments ────────────────────────────────────────────────────────
    // Doctrine references live in comments all over this codebase ("§5 ladder: accent 0.15"), and
    // matching those would make the gate cry wolf. A gate that cries wolf gets deleted, so strip
    // comments before matching anything.
    private fun codeLines(file: File): List<Pair<Int, String>> {
        var inBlock = false
        return file.readLines().mapIndexed { i, raw ->
            var line = raw
            if (inBlock) {
                val end = line.indexOf("*/")
                if (end < 0) return@mapIndexed (i + 1) to ""
                line = line.substring(end + 2); inBlock = false
            }
            while (true) {
                val start = line.indexOf("/*")
                if (start < 0) break
                val end = line.indexOf("*/", start + 2)
                if (end < 0) { line = line.substring(0, start); inBlock = true; break }
                line = line.substring(0, start) + line.substring(end + 2)
            }
            val slashes = line.indexOf("//")
            if (slashes >= 0 && !insideString(line, slashes)) line = line.substring(0, slashes)
            (i + 1) to line
        }
    }

    /**
     * The comment-stripped file as ONE string, so patterns can span lines.
     *
     * Line-by-line matching is defeated by ordinary formatting: `title = {` with `Text(` wrapped
     * onto the next line reads as compliant, as does `maxLines =` split from its `1`. There are no
     * such splits in the tree today, but a redesign that reformats a long call would introduce them
     * silently, and a rule that a line break disables is not a rule.
     */
    private fun joined(lines: List<Pair<Int, String>>): Pair<String, (Int) -> Int> {
        val sb = StringBuilder()
        val starts = ArrayList<Int>(lines.size)
        val nums = ArrayList<Int>(lines.size)
        for ((n, code) in lines) {
            starts += sb.length; nums += n
            sb.append(code).append('\n')
        }
        val text = sb.toString()
        val lineAt = { offset: Int ->
            var lo = 0; var hi = starts.size - 1; var ans = 0
            while (lo <= hi) {
                val mid = (lo + hi) / 2
                if (starts[mid] <= offset) { ans = mid; lo = mid + 1 } else hi = mid - 1
            }
            nums.getOrElse(ans) { 1 }
        }
        return text to lineAt
    }

    private fun insideString(line: String, index: Int): Boolean {
        var quotes = 0
        var i = 0
        while (i < index) {
            if (line[i] == '\\') { i += 2; continue }
            if (line[i] == '"') quotes++
            i++
        }
        return quotes % 2 == 1
    }

    /**
     * String literals on a line, found by scanning rather than by regex: the obvious pattern
     * (`"(?:[^"\\]|\\.)*"`) backtracks catastrophically on long Compose lines and blew the stack.
     */
    private fun stringLiterals(line: String): List<String> {
        val out = mutableListOf<String>()
        var i = 0
        while (i < line.length) {
            if (line[i] != '"') { i++; continue }
            val start = ++i
            while (i < line.length && line[i] != '"') {
                if (line[i] == '\\') i++
                i++
            }
            if (i >= line.length) break            // unterminated (raw string spanning lines)
            out += line.substring(start, i)
            i++
        }
        return out
    }

    // ── Patterns ───────────────────────────────────────────────────────────────────────────────

    private val ALPHA = Regex("""alpha\s*=\s*(\d*\.?\d+)f?""")

    /**
     * §5's two named exceptions to the ladder: a text-field **placeholder** may dim below the muted
     * floor (it is a ghost affordance, not content), and a **gradient or scrim** interpolates freely
     * between rungs. Nothing else.
     */
    private val LADDER_EXEMPT = Regex("""(?i)placeholder|gradient|scrim|Brush\.""")

    /**
     * Files that OWN a behaviour the doctrine sanctions, so a hit there is doctrine rather than debt.
     *
     * The allowlist should only ever contain things that ought to change. Leaving these in it made
     * the debt figure lie in the other direction — it counted the shared implementation of a rule as
     * a violation of that rule, and no amount of cleanup could ever retire the entry.
     */
    private val SANCTIONED = mapOf(
        // §9 restores the ripple under TalkBack, and this is where that fallback lives.
        "ripple" to setOf("ui/common/BounceClick.kt"),
        // §8: the app's ONE Undo snackbar is rendered here. Every other SnackbarHostState is debt.
        "snackbar-host" to setOf("ui/common/SnackbarControllerHost.kt"),
        // §5/§14: confetti is the one deliberately polychrome moment, and its palette is literal.
        "raw-color" to setOf("ui/common/ConfettiOverlay.kt"),
    )

    private fun sanctioned(rule: String, path: String) = SANCTIONED[rule]?.contains(path) == true

    /** Every file SANCTIONED names, for the test that keeps these paths from going stale. */
    fun sanctionedPaths(): List<String> = SANCTIONED.values.flatten().distinct()
    private val RAW_COLOR = Regex("""Color\(0x""")
    private val INLINE_FONT_SIZE = Regex("""fontSize\s*=""")
    private val DIVIDER = Regex("""\b(Horizontal|Vertical)?Divider\(""")
    private val MAX_LINES_ONE = Regex("""maxLines\s*=\s*1\b""")
    private val LITERAL_DURATION = Regex("""(tween\(\s*\d+|durationMillis\s*=\s*\d+)""")
    private val TEXT_TITLE = Regex("""title\s*=\s*\{\s*Text\(""")

    // Taste rules added 2026-07-24. The first three had ZERO occurrences when introduced, so they
    // cost nothing today and exist purely to stop the doctrine's most important bans regressing.
    private val M3_CARD = Regex("""(?<![A-Za-z])(Elevated|Outlined)?Card\(""")
    private val SPINNER = Regex("""(Circular|Linear)ProgressIndicator""")
    private val RTL_UNSAFE = Regex("""Alignment\.Absolute|Alignment\.(Left|Right)\b|TextAlign\.(Left|Right)\b""")
    private val TOAST = Regex("""\bToast[.(]""")
    private val SNACKBAR_HOST = Regex("""SnackbarHostState""")
    private val RIPPLE = Regex("""[Rr]ipple""")
    // `[({]`, not just `(`: Kotlin's trailing-lambda form `.clickable { … }` has no parenthesis at
    // all, and 24 unlabelled tap targets were invisible to this rule until that was noticed.
    private val UNLABELLED_CLICKABLE = Regex("""\.clickable\s*[({]""")

    // §11 — hype and bro-speak. "legend" is deliberately absent: chart legends are a real thing in
    // this app and the word appears in `EditorialLegend` labels.
    private val HYPE = Regex(
        """(?i)\b(beast|crush(ed|ing)?|smash(ed)?|killing it|nailed it|moves the needle|let's go|amazing|incredible|epic|insane)\b"""
    )
    private val PAREN_PLURAL = Regex("""\(s\)""")

    // ── Scan ───────────────────────────────────────────────────────────────────────────────────

    /** Scans every root and returns every violation, allowlisted or not. */
    fun scan(): List<Violation> {
        val out = mutableListOf<Violation>()
        for (root in roots.filter { it.dir.isDirectory }) {
            for (file in sources(root)) {
                val path = relative(file, root)
                val isTheme = path.startsWith("ui/theme/")

                val lines = codeLines(file)

                // Rules whose pattern can legitimately span lines run against the whole
                // comment-stripped file, so a wrapped call cannot slip past them.
                if (Scope.LAYOUT in root.scopes) {
                    val (text, lineAt) = joined(lines)
                    // §4.6 — the top bar carries the wordmark, never the screen's own name.
                    TEXT_TITLE.findAll(text).forEach {
                        out += Violation("screen-name-title", path, "title={Text(", lineAt(it.range.first))
                    }
                    // §14 — user content must wrap at large font scales, not truncate.
                    MAX_LINES_ONE.findAll(text).forEach {
                        out += Violation("max-lines", path, "maxLines=1", lineAt(it.range.first))
                    }
                    // §9 — durations come from ForgeMotion, which also applies the system
                    // reduce-motion preference. A literal tween silently ignores it.
                    if (!isTheme) LITERAL_DURATION.findAll(text).forEach {
                        out += Violation("literal-duration", path, "literal duration", lineAt(it.range.first))
                    }
                }

                for ((n, code) in lines) {
                    if (code.isBlank()) continue

                    if (Scope.COLOR in root.scopes) {
                        // §5 — the intensity ladder is the only allowed set of alphas, with exactly
                        // two named exceptions. Encoding them here rather than in the allowlist
                        // keeps them as doctrine (permanent, explained) instead of debt (temporary,
                        // unexplained) — and stops a regenerated baseline from losing the reason.
                        if (!LADDER_EXEMPT.containsMatchIn(code)) {
                            ALPHA.findAll(code).forEach { m ->
                                val v = m.groupValues[1]
                                if (v !in ALLOWED_ALPHA) out += Violation("alpha", path, v, n)
                            }
                        }
                        // §5 — colours come from the scheme, never a raw hex. `ui/theme/` defines
                        // them, so it is the one place a literal belongs.
                        if (!isTheme && RAW_COLOR.containsMatchIn(code)) {
                            out += Violation("raw-color", path, "Color(0x", n)
                        }
                    }

                    if (Scope.LAYOUT in root.scopes) {
                        // §6 — the three type voices ARE the meaning system; an inline size opts out.
                        if (!isTheme && INLINE_FONT_SIZE.containsMatchIn(code)) {
                            out += Violation("font-size", path, "fontSize=", n)
                        }
                        // §1/§7 — a line exists only as data. Sections separate by air.
                        if (DIVIDER.containsMatchIn(code)) {
                            out += Violation("divider", path, "Divider(", n)
                        }
                        // §1 — no boxed passive content. A box is a promise of a tap.
                        if (M3_CARD.containsMatchIn(code)) {
                            out += Violation("m3-card", path, "Card(", n)
                        }
                        // §12 — the local DB is instant. Spinners promise latency that isn't there.
                        if (SPINNER.containsMatchIn(code)) {
                            out += Violation("spinner", path, "ProgressIndicator", n)
                        }
                        // §14 — layouts stay RTL-correct: start/end, never left/right.
                        if (RTL_UNSAFE.containsMatchIn(code)) {
                            out += Violation("rtl", path, "left/right alignment", n)
                        }
                        // §12 — no success toasts for what the UI already shows.
                        if (TOAST.containsMatchIn(code)) {
                            out += Violation("toast", path, "Toast", n)
                        }
                        // §8 — one Undo snackbar for the whole app, via SnackbarController.
                        if (SNACKBAR_HOST.containsMatchIn(code)) {
                            out += Violation("snackbar-host", path, "SnackbarHostState", n)
                        }
                        // §9 — press is a bounce, not a ripple (TalkBack restores it automatically).
                        if (RIPPLE.containsMatchIn(code)) {
                            out += Violation("ripple", path, "ripple", n)
                        }
                        // §14 — a tappable element announces itself; use clickableLabeled.
                        if (UNLABELLED_CLICKABLE.containsMatchIn(code) &&
                            !code.contains("onClickLabel")
                        ) {
                            out += Violation("unlabelled-clickable", path, ".clickable(", n)
                        }
                    }

                    if (Scope.COPY in root.scopes) {
                        // §11 — banned in any rendered string.
                        stringLiterals(code).forEach { s ->
                            if (s.contains('!')) out += Violation("bang", path, "! in string", n)
                            if (s.contains('—')) out += Violation("em-dash", path, "em dash in string", n)
                            HYPE.find(s)?.let {
                                out += Violation("hype", path, "\"${it.value}\"", n)
                            }
                            if (PAREN_PLURAL.containsMatchIn(s)) {
                                out += Violation("paren-plural", path, "(s)", n)
                            }
                        }
                    }
                }
            }
        }
        // Drop hits in the files that own a sanctioned behaviour (see SANCTIONED).
        return out.filterNot { sanctioned(it.rule, it.path) }
    }

    // ── Allowlist ──────────────────────────────────────────────────────────────────────────────

    private val allowlistFile: File get() = File(moduleRoot, "src/test/resources/design-allowlist.txt")

    /** key ("rule path token") → the exact number of violations frozen for it. */
    fun allowlist(): Map<String, Int> =
        if (!allowlistFile.exists()) emptyMap()
        else allowlistFile.readLines()
            .map { it.substringBefore('#').trim() }
            .filter { it.isNotEmpty() }
            .mapNotNull { line ->
                // "<count> <rule> <path> <token...>" — count first so a token containing spaces
                // ("! in string") parses without quoting.
                val parts = line.split(Regex("\\s+"), limit = 4)
                if (parts.size < 4) null
                else parts[0].toIntOrNull()?.let { n -> "${parts[1]} ${parts[2]} ${parts[3]}" to n }
            }
            .toMap()

    fun counts(violations: List<Violation>): Map<String, Int> =
        violations.groupingBy { it.key }.eachCount()

    /**
     * Lower every count that has fallen, and refuse to raise any.
     *
     * The safe companion to [writeAllowlist] for a redesign. Cleaning up a screen makes the gate
     * fail with "debt was paid down, lower these numbers", which is correct but becomes a slog
     * across thirty files — and the tempting escape is a full regenerate, which would silently
     * swallow any NEW violation introduced in the same pass. This lowers only, and reports what it
     * refused to touch, so a redesign can bank its wins without buying a blind spot.
     *
     * Returns a human-readable report; throws if new violations exist.
     */
    fun payDownAllowlist(violations: List<Violation>): String {
        val actual = counts(violations)
        val allowed = allowlist()
        val added = (actual.keys + allowed.keys).mapNotNull { b ->
            val now = actual[b] ?: 0; val was = allowed[b] ?: 0
            if (now > was) "  $b: $was allowed, $now found" else null
        }
        if (added.isNotEmpty()) {
            error(
                "\n\nRefusing to pay down: these are NEW violations, not fixes. Address them first, " +
                    "or allowlist them deliberately with a reason:\n" + added.sorted().joinToString("\n") + "\n"
            )
        }
        val lowered = (actual.keys + allowed.keys).mapNotNull { b ->
            val now = actual[b] ?: 0; val was = allowed[b] ?: 0
            if (now < was) "  $b: $was -> $now" + (if (now == 0) "  (line removed)" else "") else null
        }
        writeAllowlist(violations)
        return if (lowered.isEmpty()) "Nothing to pay down; the allowlist already matches the code."
        else "Paid down ${lowered.size} bucket(s), total ${allowed.values.sum()} -> " +
            "${actual.values.sum()}:\n" + lowered.sorted().joinToString("\n")
    }

    /** Regenerate the baseline wholesale. Run deliberately, never as part of a test. */
    fun writeAllowlist(violations: List<Violation>) {
        val header = """
            # Frozen design-doctrine debt — see DesignDoctrineTest.
            #
            # One line per (rule, file, token): "<count> <rule> <path> <token>". The count is EXACT.
            # Adding a violation fails the build; fixing one also fails it, with a message naming the
            # new number — so paying debt down shows in the diff instead of vanishing silently.
            #
            # Counting per TOKEN, not per file, is deliberate: it means fixing three alphas while
            # introducing a fourth cannot net out to "looks like progress".
            #
            # During a redesign, bank your cleanups safely with -Dforge.paydown=true, which lowers
            # counts but REFUSES to raise any.
            #
            # This file is debt, not permission. Lower a number whenever you touch its file.
            # Regenerate deliberately after a doctrine change:
            #
            #   gradle -p forge-android :app:testDebugUnitTest --tests '*RegenerateAllowlist*' -Dforge.regen=true
        """.trimIndent()
        val body = counts(violations).entries
            .sortedWith(compareBy({ it.key.substringBefore(' ') }, { it.key }))
            .joinToString("\n") { (key, n) -> "$n $key" }
        allowlistFile.parentFile.mkdirs()
        allowlistFile.writeText("$header\n\n$body\n")
    }
}
