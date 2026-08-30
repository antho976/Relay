package com.forge.app.ui

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Polices the doctrine itself.
 *
 * The loader skill drifted from `DESIGN.md` once already: it named the wrong wordmark, taught a
 * verdict the doctrine bans by name, and carried three off-by-one section references. Nobody
 * noticed, because nothing was checking. A doc that governs code should be governed too.
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

    /**
     * Every copy of the loader, not just the canonical one.
     *
     * `.agents/skills/forge-design/SKILL.md` was a byte-for-byte duplicate of the `.claude` router
     * with every path rewritten to a `.Codex/` prefix that does not exist in this repository. It
     * told any agent that loaded it to read the binding doctrine in full before touching UI, gave a
     * path with nothing behind it, and so handed out no doctrine at all. It sat that way because
     * this test named one file by hand; a router nobody checks is a router that drifts. Discover
     * them instead.
     */
    private val routers: List<File> by lazy {
        val found = listOf(".claude", ".agents")
            .map { File(DesignDoctrine.repoRoot, "$it/skills/forge-design/SKILL.md") }
            .filter { it.isFile }
        assertTrue(
            "\n\nNo forge-design SKILL.md found under .claude/ or .agents/ in " +
                DesignDoctrine.repoRoot.canonicalPath + "\n",
            found.isNotEmpty()
        )
        found
    }

    /** The one a UI task is meant to read; the others must forward to it rather than restate it. */
    private val canonicalRouter: File
        get() = File(DesignDoctrine.repoRoot, ".claude/skills/forge-design/SKILL.md")

    @Test
    fun theLoaderSkillPointsAtThingsThatExist() {
        assertTrue("forge-design SKILL.md missing at ${canonicalRouter.path}", canonicalRouter.isFile)
        val recipeDir = DesignDoctrine.debugSource("ui/recipes")
        val broken = mutableListOf<String>()

        routers.forEach { skill ->
            val text = skill.readText()
            val where = skill.parentFile.parentFile.parentFile.name + "/"

            Regex("""`(\w+Recipe)\.kt`""").findAll(text).map { it.groupValues[1] }.toSortedSet()
                .filterNot { File(recipeDir, "$it.kt").isFile }
                .forEach { broken += "$where routes to recipe $it.kt, which does not exist" }

            Regex("""design/([A-Z]+\.md)""").findAll(text).map { it.groupValues[1] }.toSortedSet()
                .filterNot { DesignDoctrine.satellite(it).isFile }
                .forEach { broken += "$where routes to satellite design/$it, which does not exist" }

            // Any backtick-quoted repo-relative path the router hands out has to resolve. This is
            // the check that would have caught the `.Codex/` prefix on the day it was written.
            Regex("""`(\.?[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)+/?)`""").findAll(text)
                .map { it.groupValues[1] }
                // Repo-root-relative only: the router also quotes module-relative paths for the
                // Gradle commands in §4, which resolve under forge-android/ rather than here.
                .filter { it.endsWith(".md") || it.endsWith(".kt") || it.endsWith("/") }
                .toSortedSet()
                .filterNot { File(DesignDoctrine.repoRoot, it.trimEnd('/')).exists() }
                .forEach { broken += "$where routes to $it, which does not exist" }
        }

        assertTrue(
            "\n\nA design router points at files that are not in this repository. An agent that " +
                "follows it reads nothing and writes UI with no doctrine loaded:\n" +
                broken.joinToString("\n") { "  $it" } + "\n",
            broken.isEmpty()
        )
    }

    /**
     * A second copy of the doctrine is a second thing to drift, and the copy is always the one that
     * loses. Non-canonical routers forward; they do not restate.
     */
    @Test
    fun onlyOneRouterCarriesTheRouting() {
        val duplicates = routers.filter { it != canonicalRouter }
            .filterNot { it.readText().contains(".claude/skills/forge-design/SKILL.md") }
            .map { it.toRelativeString(DesignDoctrine.repoRoot) }
        assertTrue(
            "\n\nThese routers neither are the canonical one nor forward to it, so they are a " +
                "second copy of the doctrine's entry point: $duplicates\n" +
                "Replace the body with a pointer at .claude/skills/forge-design/SKILL.md.\n",
            duplicates.isEmpty()
        )
    }

    /**
     * The loader is a router, not a summary — that is precisely why it drifted last time. If it
     * starts restating rules it will disagree with the doctrine again, so keep it short.
     */
    @Test
    fun theLoaderStaysARouter() {
        val n = canonicalRouter.readLines().size
        // 110, raised from 80 on 2026-07-24 when the redesign workflow was added. Length is a proxy
        // for the real rule, which is that the loader ROUTES and never restates: an earlier version
        // summarised the doctrine, drifted from it, and ended up teaching a verdict §11 bans by name.
        // Process and commands are fine here; rules are not. If this fails, ask which one you added.
        assertTrue(
            "\n\nSKILL.md is $n lines. It is a ROUTER: it points at the doctrine, the recipes and the " +
                "maintenance commands, and restates no rules. If you added a RULE, it belongs in " +
                "DESIGN.md instead. If you added process and the file genuinely needs to be longer, " +
                "raise the cap here in the same commit.\n",
            n <= 110
        )
    }

    /** The wordmark is "Avex". The old loader said "Forge" for months. It left the top bar on
     *  2026-07-27 (the bell took that slot) but still plays at launch, so the name still has to be
     *  right wherever the docs mention it. */
    @Test
    fun theWordmarkIsNamedConsistently() {
        val offenders = mutableListOf<String>()
        (listOf(DesignDoctrine.designDoc) + routers).forEach { f ->
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
