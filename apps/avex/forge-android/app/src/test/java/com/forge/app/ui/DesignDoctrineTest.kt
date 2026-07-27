package com.forge.app.ui

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Enforces the mechanical half of `.claude/DESIGN.md` (see [DesignDoctrine] for why this exists).
 *
 * Each test reports only NEW violations — everything present when the gate landed is frozen in
 * `src/test/resources/design-allowlist.txt`. So: existing screens are untouched, new code complies,
 * and the debt is visible and shrinking rather than invisible and growing.
 *
 * To regenerate the baseline after a deliberate doctrine change, run [RegenerateAllowlist] below.
 */
class DesignDoctrineTest {

    private val violations = DesignDoctrine.scan()
    private val actual = DesignDoctrine.counts(violations)
    private val allowed = DesignDoctrine.allowlist()

    private fun check(rule: String, explanation: String) {
        val buckets = (actual.keys + allowed.keys).filter { it.startsWith("$rule ") }
        val added = mutableListOf<String>()
        val fixed = mutableListOf<String>()
        for (bucket in buckets.sorted()) {
            val now = actual[bucket] ?: 0
            val was = allowed[bucket] ?: 0
            if (now > was) {
                val examples = violations.filter { it.key == bucket }.take(4)
                added += "  $bucket: $was allowed, $now found\n" +
                    examples.joinToString("\n") { "      $it" }
            } else if (now < was) {
                fixed += "  $bucket: $was -> $now"
            }
        }
        val message = buildString {
            append("\n\n$explanation\n")
            if (added.isNotEmpty()) {
                append("\nNEW violations — fix them, or if this is a sanctioned exception, raise the\n")
                append("count in src/test/resources/design-allowlist.txt with a # comment saying why:\n")
                append(added.joinToString("\n"))
                append("\n")
            }
            if (fixed.isNotEmpty()) {
                append("\nDebt was PAID DOWN here. Lower these numbers in ")
                append("src/test/resources/design-allowlist.txt\nso the ratchet holds ")
                append("(delete the line entirely when it reaches 0):\n")
                append(fixed.joinToString("\n"))
                append("\n")
            }
        }
        assertTrue(message, added.isEmpty() && fixed.isEmpty())
    }

    @Test
    fun onlyLadderAlphas() = check(
        "alpha",
        "DESIGN §5 — the intensity ladder is the only allowed set of alphas: " +
            "1.0 / 0.7 / 0.65 / 0.6 / 0.35 / 0.25 / 0.15. " +
            "muted 0.65 is a hard floor, measured 4.54:1 on Pearl; 0.6 drops to 4.05:1 and fails AA. " +
            "A one-off alpha reads as 'slightly different' rather than as a rung, which is how the " +
            "system stops meaning anything."
    )

    @Test
    fun coloursComeFromTheScheme() = check(
        "raw-color",
        "DESIGN §5 — colours come from MaterialTheme.colorScheme, never a raw hex. A hardcoded " +
            "colour ignores the user's accent, monochrome mode and the AMOLED palette."
    )

    @Test
    fun typeComesFromTheScale() = check(
        "font-size",
        "DESIGN §6 — always take a style from MaterialTheme.typography; never set fontSize at a " +
            "call site. The three voices (serif / sans / mono) ARE the meaning system, so an inline " +
            "size opts out of it. Only sanctioned off-scale use: 8–9sp figure captions."
    )

    @Test
    fun linesAreDataOnly() = check(
        "divider",
        "DESIGN §1/§7 — a line exists only as data (chart threshold, table rule). Sections separate " +
            "by air and their mono header: 28 → header → 10 → content. Reaching for a divider is " +
            "the 'hairline habit' in design/FAILURES.md — the real problem is not enough air."
    )

    @Test
    fun userContentWrapsAtLargeFontScales() = check(
        "max-lines",
        "DESIGN §14 — no maxLines = 1 on user content (names, notes, exercise titles): it silently " +
            "truncates at large font scales. Chrome and mono labels may clamp."
    )

    @Test
    fun motionComesFromMotionKt() = check(
        "literal-duration",
        "DESIGN §9 — durations and easings come from ForgeMotion, never a literal at a call site. " +
            "ForgeMotion also applies the system reduce-motion preference automatically; a literal " +
            "tween silently ignores it."
    )

    @Test
    fun topBarNeverCarriesTheScreenName() = check(
        "screen-name-title",
        "DESIGN §4.6 — the top bar is the wordmark + ← + ≤1 action, never the screen's own name. " +
            "A screen names itself with a serif content hero, or not at all. Use " +
            "title = { ForgeWordmark() }."
    )

    @Test
    fun noExclamationMarksInRenderedStrings() = check(
        "bang",
        "DESIGN §11 — no exclamation marks in any rendered string. Confetti is the exclamation mark; " +
            "the written voice stays dry and earned."
    )

    @Test
    fun noEmDashesInRenderedStrings() = check(
        "em-dash",
        "DESIGN §11 — no em dashes in a rendered string. Split the sentence, or join with a comma " +
            "or ' · '. (Em dashes are fine in code comments and in the doctrine itself.)"
    )

    @Test
    fun noHypeInRenderedStrings() = check(
        "hype",
        "DESIGN §11 — the voice is dry, specific and earned. No bro-speak ('beast', 'crush'), no " +
            "poster clichés ('moves the needle'), no praise ungrounded in the user's data. An " +
            "understated nod is allowed only when the numbers support it."
    )

    @Test
    fun noParenPluralsInRenderedStrings() = check(
        "paren-plural",
        "DESIGN §11 'Translate the machine' — '3 session(s)' is machine prose. Branch on the count " +
            "and write the real word."
    )

    // ── Taste rules (§1, §12, §14) ─────────────────────────────────────────────────────────────
    // The first three had ZERO occurrences when introduced. They cost nothing today and exist so
    // the doctrine's most load-bearing bans can never quietly regress.

    @Test
    fun noBoxedPassiveContent() = check(
        "m3-card",
        "DESIGN §1 — no M3 Card/ElevatedCard/OutlinedCard. Content sits directly on the near-black " +
            "page; surfaces and borders are EARNED by interactivity. A box is a promise of a tap, " +
            "so a box around passive content is a lie. Modals keep their surface; nothing else does."
    )

    @Test
    fun noProgressSpinners() = check(
        "spinner",
        "DESIGN §12 — the local DB is instant, so there are no spinners or blocking loads, ever. " +
            "Screens appear with the entrance cascade. ForgeShimmer covers real latency only " +
            "(photos, Health Connect)."
    )

    @Test
    fun layoutsAreRtlSafe() = check(
        "rtl",
        "DESIGN §14 — layouts stay RTL-correct: use start/end, never left/right or " +
            "Alignment.Absolute."
    )

    @Test
    fun noToasts() = check(
        "toast",
        "DESIGN §12 — no success toasts for what the UI already shows. Reversible acts get an Undo " +
            "snackbar through SnackbarController; errors are a quiet inline line in error colour."
    )

    @Test
    fun oneSnackbarForTheWholeApp() = check(
        "snackbar-host",
        "DESIGN §8 — the app has ONE Undo snackbar: the injected SnackbarController, rendered once " +
            "at the app root by SnackbarControllerHost. Reach it with showUndo(msg) { restore }; " +
            "don't re-roll a per-screen SnackbarHostState."
    )

    @Test
    fun pressIsBounceNotRipple() = check(
        "ripple",
        "DESIGN §9 — press is a bounce (scale 0.97), not a ripple. bounceClick restores the ripple " +
            "automatically under TalkBack, so the accessible path is already handled."
    )

    @Test
    fun tappablesAnnounceThemselves() = check(
        "unlabelled-clickable",
        "DESIGN §14 — a tappable element must announce itself to TalkBack. Use clickableLabeled(" +
            "label) or bounceClick rather than a bare Modifier.clickable."
    )

    // ── The gate must actually be looking ──────────────────────────────────────────────────────

    /**
     * A gate that scans nothing passes everything. This caught a real bug: `File(".").absoluteFile`
     * keeps its trailing "/.", so `parentFile` returned the module rather than the Gradle root, the
     * `:wear` root resolved to a path that did not exist, was silently filtered out, and the whole
     * watch module went unscanned while every test stayed green.
     */
    @Test
    fun everyExpectedRootResolves() {
        val missing = DesignDoctrine.requiredRootLabels() - DesignDoctrine.resolvedRootLabels()
        assertTrue(
            "\n\nThese scan roots did not resolve on disk, so their code is NOT being checked: " +
                "$missing\nA root that silently vanishes makes this whole suite pass vacuously. " +
                "Fix the path in DesignDoctrine.roots.\n",
            missing.isEmpty()
        )
    }

    @Test
    fun everyRootActuallyContainsSources() {
        val empty = DesignDoctrine.scannedFileCounts().filterValues { it == 0 }.keys
        assertTrue(
            "\n\nThese roots resolved but contained no .kt files: $empty\nEither the path is wrong " +
                "or the module moved. Scanning zero files is indistinguishable from compliance.\n",
            empty.isEmpty()
        )
    }

    /**
     * A sanctioned exemption names a file by path. Rename or move that file and the exemption stops
     * applying silently — the rule starts firing on a behaviour the doctrine explicitly allows, and
     * the fix looks like "add it to the allowlist", which buries doctrine in the debt pile again.
     */
    @Test
    fun sanctionedExemptionsPointAtRealFiles() {
        val missing = DesignDoctrine.sanctionedPaths()
            .filterNot { DesignDoctrine.appSource(it).isFile }
        assertTrue(
            "\n\nThese files are exempted from a rule in DesignDoctrine.SANCTIONED but no longer " +
                "exist: $missing\nUpdate the path, or drop the exemption if the behaviour moved.\n",
            missing.isEmpty()
        )
    }

    // ── The ratchet ────────────────────────────────────────────────────────────────────────────

    @Test
    fun allowlistNeverGrows() {
        val total = allowed.values.sum()
        assertTrue(
            "\n\nTotal frozen design debt is $total, past the recorded baseline of " +
                "${DesignDoctrine.ALLOWLIST_BASELINE}. This number is only meant to fall. Fix the " +
                "violation rather than allowlisting it; if an exception is genuinely permanent, " +
                "raise ALLOWLIST_BASELINE in the same commit so the decision is visible in review.\n",
            total <= DesignDoctrine.ALLOWLIST_BASELINE
        )
    }
}

/**
 * Not a test — a maintenance entry point. Regenerates the frozen baseline from the current tree.
 *
 * Run deliberately after a doctrine change, never to make a red build go green:
 *
 *     gradle -p forge-android :app:testDebugUnitTest --tests '*RegenerateAllowlist*' -Dforge.regen=true
 *
 * It no-ops without the system property so it can't fire by accident in CI.
 */
class RegenerateAllowlist {

    /**
     * **Use this one during a redesign.** Lowers every count that fell, refuses to raise any:
     *
     *     gradle -p forge-android :app:testDebugUnitTest --tests '*RegenerateAllowlist*' -Dforge.paydown=true
     *
     * Safe to run after cleaning up screens, because it cannot hide a new violation introduced in
     * the same pass — it fails instead.
     */
    @Test
    fun payDown() {
        if (System.getProperty("forge.paydown") != "true") return
        println(DesignDoctrine.payDownAllowlist(DesignDoctrine.scan()))
    }

    /**
     * Wholesale re-baseline. Only after a deliberate doctrine change (a new rule, a changed ladder)
     * — never to make a red build green, because it accepts new violations silently:
     *
     *     gradle -p forge-android :app:testDebugUnitTest --tests '*RegenerateAllowlist*' -Dforge.regen=true
     */
    @Test
    fun regenerate() {
        if (System.getProperty("forge.regen") != "true") return
        val found = DesignDoctrine.scan()
        DesignDoctrine.writeAllowlist(found)
        println(
            "design-allowlist.txt rewritten: ${found.size} violations across " +
                "${DesignDoctrine.counts(found).size} buckets"
        )
    }
}
