package com.forge.app.data.repo

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.MIGRATION_39_40
import com.forge.app.data.db.entities.ProgramDay
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.program.Equipment
import com.forge.app.program.GenerationParams
import com.forge.app.program.Program
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * A regenerate that keeps a day keeps the name the user gave it. Migration 39→40 moved the legacy
 * day-name overrides into `program_day.name`; before it, they lived in their own table and survived
 * every regenerate, so the next deload must not quietly reset them.
 */
@RunWith(RobolectricTestRunner::class)
class RegenerateKeepsDayNamesTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val db: ForgeDatabase = inMemoryForgeDb()
    private val settings = SettingsRepository(context, Clock { NOW })
    private val programRepository = ProgramRepository(
        database = db,
        dao = db.programDao(),
        customizationDao = db.programCustomizationDao(),
        programCustomizationRepo = ProgramCustomizationRepository(db.programCustomizationDao()),
        exerciseCustomizationDao = db.exerciseCustomizationDao(),
        sessionDao = db.sessionDao(),
        coachDao = db.coachDao(),
        settings = settings,
        adaptationRepositoryLazy = dagger.Lazy<AdaptationRepository> { error("not used by generate") },
        clock = Clock { NOW },
        context = context
    )

    // generate() loads its program into the process-wide facade; put back what the next class expects.
    private val original = Program.days

    @After fun tearDown() { Program.setActive(original); db.close() }

    private suspend fun generate(days: Int, deload: Boolean = false) = programRepository.generate(
        GenerationParams(days, deload = deload), Equipment.entries.toSet(), emptySet(), emptySet(), keepPicks = true
    )

    private suspend fun names(): Map<String, String> = db.programDao().days().associate { it.id to it.name }

    @Test
    fun `a folded legacy name survives the deload and the restore after it`() = runBlocking {
        generate(4)
        val sql = db.openHelper.writableDatabase
        sql.execSQL("INSERT INTO day_name_override (day_key, custom_name) VALUES ('upper-a', 'Chest day')")
        MIGRATION_39_40.migrate(sql)

        generate(4, deload = true)
        assertEquals("Chest day", names()["upper-a"])
        generate(4)
        assertEquals("Chest day", names()["upper-a"])
        // Days the user never renamed still read the generator's names.
        assertEquals("Lower A", names()["lower-a"])
    }

    @Test
    fun `a new split takes the generator's names`() = runBlocking {
        generate(4)
        db.openHelper.writableDatabase.execSQL("UPDATE program_day SET name = 'Chest day' WHERE id = 'upper-a'")

        generate(3)

        assertEquals(setOf("Full Body A", "Full Body B", "Full Body C"), names().values.toSet())
    }

    @Test
    fun `a day the builder turned into another type is a different day`() {
        val legDay = ProgramDay("upper-a", 0, "Leg day", "LEGS", "#FFFFFF", "legs")
        assertNull(keptDayName(listOf(legDay), "upper-a", "upper-a"))
        assertEquals("Chest day", keptDayName(listOf(legDay.copy(name = "Chest day", archetype = "upper")), "upper-a", "upper-a"))
        assertNull(keptDayName(listOf(legDay.copy(name = "  ", archetype = "upper-a")), "upper-a", "upper-a"))
        assertNull(keptDayName(emptyList(), "upper-a", "upper-a"))
    }

    private companion object {
        const val NOW = 1_700_000_000_000L
    }
}
