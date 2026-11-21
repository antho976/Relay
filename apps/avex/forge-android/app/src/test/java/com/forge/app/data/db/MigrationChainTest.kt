package com.forge.app.data.db

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The migration chain's STRUCTURE, checked on the JVM so it runs on every push.
 *
 * The deep per-migration validation — create v(n), migrate, diff against the exported schema — is
 * [com.forge.app.data.db.MigrationTest], and it can only run on a device: Room's MigrationTestHelper
 * reads the exported schemas from the instrumentation context's assets, which Robolectric does not
 * populate from the app. So that suite runs in CI's emulator job, minutes behind everything else,
 * and until recently did not run in CI at all.
 *
 * Everything checked HERE needs no database at all. It is the bookkeeping around the chain, and it
 * is exactly what gets forgotten: bumping `version` without writing the migration, writing the
 * migration without registering it in ALL_MIGRATIONS, registering it without exporting the schema,
 * or exporting it without adding the device-side case. Each of those ships a build that wipes or
 * crashes an upgrading user's database while a fresh install works perfectly — the failure mode
 * nobody developing the app ever sees.
 *
 * `ForgeDatabase`'s own KDoc states the rule this enforces: "every change needs a real Migration in
 * Migrations, a bumped version here, an exported schema JSON, and a case in MigrationTest."
 */
class MigrationChainTest {

    /** The oldest version that migrates rather than resetting — see DatabaseModule's fallback. */
    private val firstLockedVersion = 12

    /**
     * Read out of the source rather than off the annotation: Room's `@Database` is retained at
     * CLASS level, not RUNTIME, so reflection cannot see it at all. Parsing the declaration is also
     * the more honest check — what a reviewer reads in the file is what this compares against.
     */
    private val declaredVersion: Int by lazy {
        val source = File(appModule, "src/main/java/com/forge/app/data/db/ForgeDatabase.kt")
        require(source.isFile) { "ForgeDatabase.kt not found at ${source.path}" }
        Regex("""\bversion\s*=\s*(\d+)""").find(source.readText())
            ?.groupValues?.get(1)?.toInt()
            ?: error("Could not read `version =` out of ${source.path}")
    }

    private val appModule: File by lazy {
        generateSequence(File(".").canonicalFile) { it.parentFile }
            .firstOrNull { File(it, "schemas/com.forge.app.data.db.ForgeDatabase").isDirectory }
            ?: error("Could not locate the :app module from ${File(".").canonicalPath}")
    }

    private val schemaDir: File get() = File(appModule, "schemas/com.forge.app.data.db.ForgeDatabase")

    private val exportedVersions: List<Int> by lazy {
        schemaDir.listFiles().orEmpty()
            .mapNotNull { it.name.removeSuffix(".json").toIntOrNull() }
            .sorted()
    }

    @Test
    fun theChainIsContiguousFromTheLockedVersionToTheDeclaredOne() {
        val steps = ALL_MIGRATIONS.map { it.startVersion to it.endVersion }

        assertEquals(
            "the chain must start where destructive fallback stops",
            firstLockedVersion, steps.first().first
        )
        assertEquals(
            "the chain must reach the version @Database declares, or an upgrade finds no path " +
                "and Room throws IllegalStateException on first launch",
            declaredVersion, steps.last().second
        )

        steps.zipWithNext { (_, endOfPrevious), (startOfNext, _) ->
            assertEquals(
                "gap in the migration chain: nothing migrates $endOfPrevious -> $startOfNext",
                endOfPrevious, startOfNext
            )
        }
        steps.forEach { (from, to) ->
            assertEquals("migrations must move exactly one version at a time, got $from -> $to", from + 1, to)
        }
        assertEquals(
            "a migration is registered twice in ALL_MIGRATIONS",
            steps.size, steps.toSet().size
        )
        assertEquals(
            "one migration per version step between $firstLockedVersion and $declaredVersion",
            declaredVersion - firstLockedVersion, steps.size
        )
    }

    @Test
    fun everyVersionInTheChainHasAnExportedSchema() {
        // Room diffs against these files. A missing one turns the device-side validation into a
        // silent no-op for that step.
        val missing = (firstLockedVersion..declaredVersion).filterNot { it in exportedVersions }
        assertTrue(
            "no exported schema JSON for version(s) $missing in ${schemaDir.path} — " +
                "build the app once after bumping the version so KSP writes them",
            missing.isEmpty()
        )
    }

    @Test
    fun theNewestExportedSchemaMatchesTheDeclaredVersion() {
        // Catches the reverse mistake: the version was bumped and the schema exported, but the app
        // was never rebuilt after the LAST edit, so the newest JSON describes the previous shape.
        assertEquals(
            "the newest exported schema should be $declaredVersion",
            declaredVersion, exportedVersions.last()
        )
    }

    @Test
    fun everyMigrationHasACaseInTheDeviceSideSuite() {
        // The rule ForgeDatabase's KDoc states and nothing enforced. A migration with no case is a
        // migration whose SQL has never been run against the schema it claims to produce.
        val suite = File(
            appModule,
            "src/androidTest/java/com/forge/app/data/db/MigrationTest.kt"
        )
        assertTrue("MigrationTest.kt not found at ${suite.path}", suite.isFile)
        val source = suite.readText()

        val uncovered = ALL_MIGRATIONS
            .map { "MIGRATION_${it.startVersion}_${it.endVersion}" }
            .filterNot { it in source }
        assertTrue(
            "no case in MigrationTest for: $uncovered. Add one — it is the only thing that runs " +
                "the migration's SQL against the schema it claims to produce.",
            uncovered.isEmpty()
        )
    }

    @Test
    fun noVersionIsBothMigratableAndDestructivelyReset() {
        // DatabaseModule destructively falls back from 1..11 only. If a migration ever started
        // below 12, a user on that version would be reset despite a migration existing for them —
        // data loss that looks like a fresh install.
        val destructivelyReset = 1..(firstLockedVersion - 1)
        val overlapping = ALL_MIGRATIONS.filter { it.startVersion in destructivelyReset }
        assertTrue(
            "these migrations start inside the destructive-fallback range and would never run: " +
                overlapping.map { "${it.startVersion}->${it.endVersion}" },
            overlapping.isEmpty()
        )
    }
}
