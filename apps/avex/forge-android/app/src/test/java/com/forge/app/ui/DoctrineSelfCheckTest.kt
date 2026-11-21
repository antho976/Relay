package com.forge.app.ui

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Polices the doctrine itself.
 *
 * The old forge-design loader skill drifted from `DESIGN.md`: it named the wrong wordmark, taught a
 * verdict the doctrine bans by name, and carried three off-by-one section references. That skill is
 * gone (2026-09-27, design work now loads impeccable, which reads PRODUCT.md and this doctrine),
 * but a doc that governs code should still be governed.
 */
class DoctrineSelfCheckTest {

    private val doc: String by lazy { DesignDoctrine.designDoc.readText() }
    private val lines: List<String> by lazy { doc.lines() }

    /** DESIGN §16 states this cap. Exceeding it quietly is how the pre-split doctrine hit 2.5×. */
    private val lineCap = 420

    @Test
    fun doctrineRespectsItsOwnLineCap() {
        val n = DesignDoctrine.designDoc.readLines().size
        assertTrue(
            "\n\nDESIGN.md is $n lines, over its $lineCap-line cap.\n" +
                "Adding a rule means finding one that can leave, move to a satellite, or become a " +
                "test. If the cap is genuinely wrong, change it in §16 AND here in the same " +
                "commit — never let the file quietly exceed it.\n",
            n <= lineCap
        )
    }

    @Test
    fun theDocumentedCapMatchesTheEnforcedCap() {
        val stated = Regex("""[Bb]udget: (\d+) lines""").find(doc)?.groupValues?.get(1)?.toInt()
        assertTrue(
            "\n\n§16 must state the same cap this test enforces ($lineCap). Found: $stated\n",
            stated == lineCap
        )
    }

    @Test
    fun everySectionReferenceResolves() {
        val headings = lines.mapNotNull { Regex("""^## (\d+)\.""").find(it)?.groupValues?.get(1)?.toInt() }
            .toSet()
        // §4.3 style sub-references resolve to their parent section.
        val referenced = Regex("""§(\d+)""").findAll(doc).map { it.groupValues[1].toInt() }.toSet()
        val dangling = referenced - headings
        assertTrue(
            "\n\nDESIGN.md references sections that do not exist: " +
                dangling.sorted().joinToString { "§$it" } +
                "\nHeadings present: " + headings.sorted().joinToString { "§$it" } + "\n",
            dangling.isEmpty()
        )
    }

    /**
     * `§4.10` resolves only while §4 still has ten numbered items. Reorder or trim that list and
     * every sub-reference silently points at nothing, which the parent-section check above cannot
     * see because §4 itself still exists.
     */
    @Test
    fun everySubReferenceResolves() {
        val itemCounts = Regex("""^## (\d+)\.""", RegexOption.MULTILINE).findAll(doc).associate { m ->
            val n = m.groupValues[1].toInt()
            val rest = doc.substring(m.range.last)
            val nextHeading = Regex("""^## \d+\.""", RegexOption.MULTILINE).find(rest.drop(1))
            val body = if (nextHeading == null) rest else rest.take(nextHeading.range.first)
            n to Regex("""^\d+\. \*\*""", RegexOption.MULTILINE).findAll(body).count()
        }
        val dangling = Regex("""§(\d+)\.(\d+)""").findAll(doc)
            .map { it.groupValues[1].toInt() to it.groupValues[2].toInt() }
            .filter { (sec, sub) -> (itemCounts[sec] ?: 0) < sub }
            .map { (sec, sub) -> "§$sec.$sub (§$sec has ${itemCounts[sec] ?: 0} numbered items)" }
            .distinct().toList()
        assertTrue(
            "\n\nDESIGN.md references sub-items that do not exist:\n" +
                dangling.joinToString("\n") { "  $it" } + "\n",
            dangling.isEmpty()
        )
    }

    @Test
    fun everyReferencedSatelliteExists() {
        val referenced = Regex("""design/([A-Z]+\.md)""").findAll(doc)
            .map { it.groupValues[1] }.toSortedSet()
        val missing = referenced.filterNot { DesignDoctrine.satellite(it).isFile }
        assertTrue(
            "\n\nDESIGN.md points at satellites that do not exist: $missing\n",
            missing.isEmpty()
        )
        assertTrue("DESIGN.md should reference at least one satellite", referenced.isNotEmpty())
    }

    @Test
    fun everyArchetypeHasACompilingRecipe() {
        val recipeDir = DesignDoctrine.debugSource("ui/recipes")
        assertTrue("Recipes directory missing at ${recipeDir.path}", recipeDir.isDirectory)
        val present = recipeDir.listFiles { f: File -> f.extension == "kt" }.orEmpty()
            .map { it.nameWithoutExtension }.toSortedSet()

        // §3 lists six archetypes; §0 tells the reader to start from the matching recipe.
        val required = setOf(
            "OverviewRecipe", "DetailRecipe", "ListRecipe",
            "SettingsRecipe", "LiveRecipe", "ModalRecipe",
        )
        val missing = required - present
        assertTrue(
            "\n\n§0 tells every UI task to start from its archetype's recipe, but these do not " +
                "exist: $missing\nPresent: $present\n",
            missing.isEmpty()
        )
    }

    /** The wordmark is "Avex". The old loader said "Forge" for months. It left the top bar on
     *  2026-07-27 (the bell took that slot) but still plays at launch, so the name still has to be
     *  right wherever the docs mention it. */
    @Test
    fun theWordmarkIsNamedConsistently() {
        val offenders = mutableListOf<String>()
        listOf(DesignDoctrine.designDoc).forEach { f ->
            if (Regex("""[•·]\s*Forge\b""").containsMatchIn(f.readText())) {
                offenders += f.toRelativeString(DesignDoctrine.repoRoot)
            }
        }
        assertTrue(
            "\n\nThese files call the wordmark '• Forge'. It is '• Avex' (AvexWordmark renders " +
                "Avex; the package name is historical): $offenders\n",
            offenders.isEmpty()
        )
    }
}
