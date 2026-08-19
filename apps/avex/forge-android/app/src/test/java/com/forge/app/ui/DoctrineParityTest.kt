package com.forge.app.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import kotlin.math.pow

/**
 * Asserts that every VALUE `.claude/DESIGN.md` states is the value the code actually uses.
 *
 * The doctrine is only useful while it is true. Two kinds of lie are possible: the doc drifting from
 * the code (it already had — §8 claimed `statsEntrance` lives in `ui/common/`, and it did not), and
 * the code drifting from the doc (a palette tweak quietly breaking the contrast §14 promises). Both
 * are mechanical, so both are checkable, and neither should ever again depend on someone noticing.
 *
 * These tests fail in BOTH directions on purpose. If a value legitimately changes, change it in the
 * code and in the doc in the same commit — which is exactly what §16's protocol already requires.
 */
class DoctrineParityTest {

    private val doc: String by lazy { DesignDoctrine.designDoc.readText() }

    /** The body of one `## N. …` section of the doctrine. */
    private fun section(number: Int): String {
        val lines = doc.lines()
        val start = lines.indexOfFirst { it.startsWith("## $number.") }
        require(start >= 0) { "DESIGN.md has no section §$number" }
        val end = lines.drop(start + 1).indexOfFirst { it.startsWith("## ") }
        return (if (end < 0) lines.drop(start) else lines.subList(start, start + 1 + end)).joinToString("\n")
    }

    private fun read(file: File): String {
        assertTrue("Expected ${file.path} to exist", file.isFile)
        return file.readText()
    }

    /**
     * One palette token's hex, read from `Color.kt`.
     *
     * The contrast tests below used to hardcode the background and muted hexes, which is the exact thing
     * this file's own header warns against: a test carrying its own copy of a value lets the code
     * and the doc drift while the test stays green. Warming the palette (2026-08-16) proved the
     * point — every ratio moved and these tests could not have noticed. They read the real palette
     * now, so a future palette change fails LOUDLY against the doc instead of silently agreeing
     * with a stale constant.
     */
    private fun paletteHex(token: String): String =
        Regex("""val\s+$token\s*=\s*Color\(0xFF([0-9A-Fa-f]{6})\)""")
            .find(read(DesignDoctrine.appSource("ui/theme/Color.kt")))
            ?.groupValues?.get(1)?.uppercase()
            ?: error("Color.kt no longer defines $token")

    private val pageBg: String get() = paletteHex("PearlBackground")
    private val mutedTone: String get() = paletteHex("PearlMuted")

    // ── §5 Colour ──────────────────────────────────────────────────────────────────────────────

    @Test
    fun colourTokensMatchColorKt() {
        val code = read(DesignDoctrine.appSource("ui/theme/Color.kt"))
        val defined = Regex("""val\s+(\w+)\s*=\s*Color\(0xFF([0-9A-Fa-f]{6})\)""")
            .findAll(code)
            .associate { it.groupValues[1] to it.groupValues[2].uppercase() }
        // The AMOLED overrides are applied inline in ForgeTheme.kt rather than named in Color.kt,
        // so the reverse check below has to see them too or it flags them as invented.
        val themeHexes = Regex("""Color\(0xFF([0-9A-Fa-f]{6})\)""")
            .findAll(read(DesignDoctrine.appSource("ui/theme/ForgeTheme.kt")))
            .map { it.groupValues[1].uppercase() }.toSet()

        // Only the tokens §5 actually names. Light-scheme values are unshipped (SETTLED.md) and the
        // doctrine deliberately does not document them.
        val documented = listOf(
            "PearlBackground", "PearlSurface", "PearlSurfaceVar", "PearlOutline",
            "PearlOnBg", "PearlMuted", "PearlGradTop", "PearlGradBottom",
            "AccentNavy", "AccentRed", "AccentOlive", "AccentGold",
            "ForgeSuccess", "ForgeWarning", "ForgeError", "ForgePrGold", "ForgeLastGreen",
        )
        val s5 = section(5)
        val missing = documented.mapNotNull { name ->
            val hex = defined[name] ?: return@mapNotNull "$name is not defined in Color.kt"
            if (!s5.contains(hex, ignoreCase = true)) "$name = #$hex is in Color.kt but not in §5"
            else null
        }
        assertTrue(
            "\n\nDESIGN §5 must state the palette the code actually uses:\n" +
                missing.joinToString("\n") { "  $it" } + "\n",
            missing.isEmpty()
        )

        // …and the reverse: §5 may not name a colour the code does not define.
        val known = defined.values.toSet() + themeHexes
        val invented = Regex("""#([0-9A-Fa-f]{6})""").findAll(s5)
            .map { it.groupValues[1].uppercase() }
            .filter { it !in known && it != "000000" }   // AMOLED pure black is described, not a token
            .toSet()
        assertTrue(
            "\n\nDESIGN §5 names colours that do not exist in Color.kt: " +
                invented.joinToString { "#$it" } + "\n",
            invented.isEmpty()
        )
    }

    // ── §6 Type ────────────────────────────────────────────────────────────────────────────────

    @Test
    fun typeScaleMatchesTypeKt() {
        val code = read(DesignDoctrine.appSource("ui/theme/Type.kt"))
        // style name -> (family, size) — one TextStyle block per Material style.
        val styles = Regex(
            """(\w+)\s*=\s*TextStyle\((?:[^)]*?)fontFamily\s*=\s*FontFamily\.(\w+)(?:.*?)fontSize\s*=\s*(\d+)\.sp""",
            setOf(RegexOption.DOT_MATCHES_ALL)
        ).findAll(code).associate { m ->
            m.groupValues[1] to (m.groupValues[2] to m.groupValues[3].toInt())
        }
        assertTrue("Could not parse Type.kt (found ${styles.size} styles)", styles.size >= 13)

        val s6 = section(6)
        val voiceRow = mapOf(
            "Serif" to s6.lines().first { it.contains("**Serif**") },
            "SansSerif" to s6.lines().first { it.contains("**Sans**") },
            "Monospace" to s6.lines().first { it.contains("**Mono**") },
        )
        val problems = styles.mapNotNull { (style, fs) ->
            val (family, size) = fs
            val row = voiceRow[family] ?: return@mapNotNull "$style uses FontFamily.$family, which §6 does not describe"
            // Not \b…\b: §6 writes weights inline ("titleL 18M"), so a trailing letter is expected.
            if (!Regex("""(?<!\d)$size(?!\d)""").containsMatchIn(row)) {
                "$style is ${size}sp $family in Type.kt, but §6's $family row does not list $size"
            } else null
        }
        assertTrue(
            "\n\nDESIGN §6's three voices must match Type.kt:\n" +
                problems.joinToString("\n") { "  $it" } + "\n",
            problems.isEmpty()
        )
    }

    // ── §7 Shape ───────────────────────────────────────────────────────────────────────────────

    @Test
    fun shapeRadiiMatchShapeKt() {
        val code = read(DesignDoctrine.appSource("ui/theme/Shape.kt"))
        val radii = Regex("""RoundedCornerShape\((\d+)\.dp\)""")
            .findAll(code).map { it.groupValues[1].toInt() }.toSortedSet()
        val s7 = section(7)
        val documented = Regex("""Shapes? \(`Shape\.kt`\): ([\d/]+)""").find(s7)?.groupValues?.get(1)
            ?: Regex("""`Shape\.kt`\): ([\d/]+)""").find(s7)?.groupValues?.get(1)
        assertEquals(
            "\n\nDESIGN §7 must list the radii Shape.kt defines.\n",
            radii.joinToString("/"),
            documented
        )
    }

    // ── §9 Motion ──────────────────────────────────────────────────────────────────────────────

    @Test
    fun motionDurationsMatchMotionKt() {
        val code = read(DesignDoctrine.appSource("ui/theme/Motion.kt"))
        val durations = Regex("""const val Duration(\w+) = (\d+)""")
            .findAll(code).associate { it.groupValues[1] to it.groupValues[2] }
        assertTrue("Could not parse Motion.kt durations", durations.size >= 5)

        val s9 = section(9)
        // Match the LABELLED pair ("Draw 900"), not the bare number. Searching for the value alone
        // let an unrelated 900 elsewhere in §9 (the ForgeSwitch spring) satisfy the check, so
        // deleting "Draw 900" from the table passed silently.
        val missing = durations.filterNot { (name, ms) ->
            Regex("""$name\s+$ms\b""", RegexOption.IGNORE_CASE).containsMatchIn(s9)
        }
        assertTrue(
            "\n\nDESIGN §9 must state each ForgeMotion duration as a labelled pair, e.g. " +
                "\"Draw 900\". Missing or mislabelled: " +
                missing.entries.joinToString { "${it.key} ${it.value}" } + "\n",
            missing.isEmpty()
        )
    }

    // ── §8 Component inventory ─────────────────────────────────────────────────────────────────

    @Test
    fun commonInventoryMatchesUiCommon() {
        val dir = DesignDoctrine.appSource("ui/common")
        assertTrue("ui/common not found", dir.isDirectory)

        // Top-level composables only: `fun Name(` with an uppercase name and no extension receiver.
        val exported = dir.listFiles { f: File -> f.extension == "kt" }.orEmpty()
            .flatMap { f ->
                Regex("""^(?:internal |public )?fun ([A-Z]\w+)\(""", RegexOption.MULTILINE)
                    .findAll(f.readText()).map { it.groupValues[1] }
            }.toSortedSet()

        // Anywhere in the doctrine counts, not just §8: the launch primitives (AvexIntro,
        // AvexWordmark, IconLaunchScene) are documented in §9 where they belong.
        val mentioned = Regex("""`(\w+)`""").findAll(doc).map { it.groupValues[1] }.toSet()

        // Deprecated ones are excluded: §12 records their status and they are on the way out,
        // not part of the inventory.
        val deprecated = setOf("EmptyState", "FirstTouchTip")
        val unmentioned = exported - mentioned - deprecated
        assertTrue(
            "\n\nThese composables live in ui/common/ but DESIGN.md never names them, so nobody " +
                "reading the doctrine knows they exist: $unmentioned\n" +
                "Add them to §8, or move them out of ui/common/ if they are not actually shared.\n",
            unmentioned.isEmpty()
        )
    }

    @Test
    fun inventoryDoesNotClaimComponentsThatLiveElsewhere() {
        val commonNames = DesignDoctrine.appSource("ui/common")
            .listFiles { f: File -> f.extension == "kt" }.orEmpty()
            .flatMap { f ->
                Regex("""^(?:internal |public )?fun ([A-Z]\w+)\(|^fun Modifier\.(\w+)""", RegexOption.MULTILINE)
                    .findAll(f.readText())
                    .map { it.groupValues[1].ifEmpty { it.groupValues[2] } }
            }.toSet()

        // §8's opening paragraph is the inventory: it says these live in `ui/common/`. Anything it
        // names there must actually be there. This is the check that would have caught
        // `statsEntrance` being claimed by §8 while living in ui/gym/stats/components/.
        val inventoryParagraph = section(8).substringBefore("**Buttons")
        val claimed = Regex("""`(\w+)`""").findAll(inventoryParagraph).map { it.groupValues[1] }.toSet()

        // Names that are demonstrably composables/modifiers somewhere in ui/ but not in ui/common/.
        val uiRoot = DesignDoctrine.appSource("ui")
        val elsewhere = claimed.filter { name ->
            name !in commonNames &&
                uiRoot.walkTopDown().any { f ->
                    f.extension == "kt" && !f.path.contains("/common/") &&
                        Regex("""fun (Modifier\.)?$name\(""").containsMatchIn(f.readText())
                }
        }
        assertTrue(
            "\n\nDESIGN §8 lists these under `ui/common/`, but they live elsewhere in ui/: " +
                "$elsewhere\nEither promote them to ui/common/ (§2⑥ — a pattern used on a 3rd " +
                "screen gets promoted) or correct §8.\n",
            elsewhere.isEmpty()
        )
    }

    // ── Wear parity ────────────────────────────────────────────────────────────────────────────

    /**
     * `design/WEAR.md` says the watch takes the phone's palette and "the two surfaces may not
     * drift". The watch is a separate Gradle module and hardcodes its own copies of the hexes, so
     * nothing but this test connects the two.
     */
    @Test
    fun wearPaletteMatchesThePhone() {
        val phone = read(DesignDoctrine.appSource("ui/theme/Color.kt"))
        fun phoneHex(token: String) = Regex("""val\s+$token\s*=\s*Color\(0xFF([0-9A-Fa-f]{6})\)""")
            .find(phone)?.groupValues?.get(1)?.uppercase()
            ?: error("Color.kt no longer defines $token")

        val wearDir = DesignDoctrine.wearSource("")
        assertTrue("Wear sources not found at ${wearDir.path}", wearDir.isDirectory)
        val wear = wearDir.walkTopDown()
            .filter { it.isFile && it.extension == "kt" }
            .joinToString("\n") { it.readText() }

        val shared = mapOf(
            "onBg" to phoneHex("PearlOnBg"),
            "surface" to phoneHex("PearlSurface"),
            "PR gold" to phoneHex("ForgePrGold"),
        )
        val missing = shared.filterValues { hex -> !wear.contains(hex, ignoreCase = true) }
        assertTrue(
            "\n\ndesign/WEAR.md — the watch inherits the phone's palette and the two may not drift. " +
                "These phone values no longer appear anywhere in :wear: " +
                missing.entries.joinToString { "${it.key} #${it.value}" } +
                "\nEither the phone palette changed and the watch was not updated, or the watch " +
                "diverged on its own.\n",
            missing.isEmpty()
        )
    }

    // ── §14 Contrast ───────────────────────────────────────────────────────────────────────────

    private fun luminance(hex: String): Double {
        fun chan(v: Int): Double {
            val c = v / 255.0
            return if (c <= 0.03928) c / 12.92 else ((c + 0.055) / 1.055).pow(2.4)
        }
        val h = hex.removePrefix("#")
        val r = chan(h.substring(0, 2).toInt(16))
        val g = chan(h.substring(2, 4).toInt(16))
        val b = chan(h.substring(4, 6).toInt(16))
        return 0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    private fun ratio(fg: String, bg: String): Double {
        val a = luminance(fg); val b = luminance(bg)
        return (maxOf(a, b) + 0.05) / (minOf(a, b) + 0.05)
    }

    private fun blend(fg: String, bg: String, alpha: Double): String {
        fun mix(i: Int): Int {
            val f = fg.removePrefix("#").substring(i, i + 2).toInt(16)
            val b = bg.removePrefix("#").substring(i, i + 2).toInt(16)
            return Math.round(f * alpha + b * (1 - alpha)).toInt()
        }
        return "%02X%02X%02X".format(mix(0), mix(2), mix(4))
    }

    @Test
    fun mutedFloorStillClearsAA() {
        val bg = pageBg; val muted = mutedTone
        val atFloor = ratio(blend(muted, bg, 0.65), bg)
        val below = ratio(blend(muted, bg, 0.60), bg)
        assertTrue(
            "\n\nDESIGN §5/§14 — muted @0.65 is the documented hard floor and must clear AA (4.5:1). " +
                "Measured ${"%.2f".format(atFloor)}:1. If the palette changed, the floor moved and " +
                "both the ladder and §14's table need updating.\n",
            atFloor >= 4.5
        )
        assertTrue(
            "\n\nDESIGN §5 justifies the 0.65 floor by 0.60 failing AA. It now measures " +
                "${"%.2f".format(below)}:1, which passes, so the stated reason is wrong.\n",
            below < 4.5
        )
    }

    /**
     * Every ratio §14 states is recomputed from the real palette. The claims are READ FROM THE DOC,
     * not hardcoded here: a test that carries its own copy of the numbers lets the doc drift from
     * the test while both stay green, which is the exact failure mode this file exists to prevent.
     */
    @Test
    fun contrastTableMatchesTheRealPalette() {
        val bg = pageBg
        val s14 = section(14)
        val drift = mutableListOf<String>()
        fun compare(label: String, actual: Double, stated: Double) {
            if (kotlin.math.abs(actual - stated) > 0.02) {
                drift += "$label: §14 says ${stated}:1, actual ${"%.2f".format(actual)}:1"
            }
        }

        // Rows carrying an explicit hex, e.g. "| onBg `#EEEEF2` | 16.66:1 | ✓ |".
        var hexRows = 0
        s14.lines().forEach { row ->
            val hex = Regex("""#([0-9A-Fa-f]{6})""").find(row)?.groupValues?.get(1) ?: return@forEach
            if (hex.equals(bg, ignoreCase = true)) return@forEach          // the background itself
            val stated = Regex("""(\d+\.\d+):1""").find(row)?.groupValues?.get(1)?.toDouble() ?: return@forEach
            hexRows++
            compare("#$hex", ratio(hex, bg), stated)
        }

        // Alpha-blended muted rows, e.g. "| muted @0.65 | 4.54:1 |" and "muted 1.0 | 9.41:1".
        var mutedRows = 0
        Regex("""muted\s*@?(\d\.\d+)\*{0,2}\s*\|\s*\*{0,2}(\d+\.\d+):1""").findAll(s14).forEach { m ->
            val alpha = m.groupValues[1].toDouble()
            mutedRows++
            compare(
                "muted @$alpha",
                ratio(if (alpha >= 1.0) mutedTone else blend(mutedTone, bg, alpha), bg),
                m.groupValues[2].toDouble()
            )
        }
        // The justification for the floor: "0.6 gives 4.05:1".
        Regex("""(\d\.\d+) gives (\d+\.\d+):1""").findAll(s14).forEach { m ->
            mutedRows++
            compare(
                "muted @${m.groupValues[1]} (floor justification)",
                ratio(blend(mutedTone, bg, m.groupValues[1].toDouble()), bg),
                m.groupValues[2].toDouble()
            )
        }

        // The per-accent failures, e.g. "Navy 2.35 · Red 2.44 · Olive 2.81 · Gold 3.40".
        val colorKt = read(DesignDoctrine.appSource("ui/theme/Color.kt"))
        var accentRows = 0
        listOf("Navy", "Red", "Olive", "Gold").forEach { name ->
            val stated = Regex("""$name (\d+\.\d+)""").find(s14)?.groupValues?.get(1)?.toDouble() ?: return@forEach
            val hex = Regex("""val\s+Accent$name\s*=\s*Color\(0xFF([0-9A-Fa-f]{6})\)""")
                .find(colorKt)?.groupValues?.get(1) ?: return@forEach
            accentRows++
            compare("Accent$name", ratio(hex, bg), stated)
        }

        assertTrue(
            "\n\nDESIGN §14's contrast table no longer matches the palette. Recompute it, and update " +
                "the table AND design/SETTLED.md's open decisions:\n" +
                drift.joinToString("\n") { "  $it" } + "\n",
            drift.isEmpty()
        )
        // A parser that silently matches nothing would make this test pass vacuously.
        assertTrue(
            "\n\nParsed no ratios out of §14 (hex rows=$hexRows, muted rows=$mutedRows, " +
                "accent rows=$accentRows). The table's format changed and this parser no longer " +
                "reads it, so nothing is actually being verified.\n",
            hexRows >= 2 && mutedRows >= 3 && accentRows == 4
        )
    }

    @Test
    fun reservedColoursStillPassAsText() {
        val bg = pageBg
        // §5 reserves these for true states, and §14 records them as clearing AA. If a future
        // palette tweak drops one below 4.5:1, the doctrine's claim silently becomes false.
        val reserved = mapOf(
            "PR gold" to "E3B341", "LAST green" to "5BC873",
            "success" to "4CAF7D", "warning" to "CFAB47",
        )
        val failing = reserved.filterValues { ratio(it, bg) < 4.5 }
        assertTrue(
            "\n\nDESIGN §14 records these as clearing AA on Pearl; they no longer do: " +
                failing.entries.joinToString { "${it.key} ${"%.2f".format(ratio(it.value, bg))}:1" } + "\n",
            failing.isEmpty()
        )
    }
}
