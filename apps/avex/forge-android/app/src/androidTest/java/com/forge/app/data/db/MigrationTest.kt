package com.forge.app.data.db

import androidx.room.testing.MigrationTestHelper
import androidx.sqlite.db.framework.FrameworkSQLiteOpenHelperFactory
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

/**
 * Validates the locked migrations against the exported schemas. Runs on a device/emulator
 * (`./gradlew connectedAndroidTest`). Add a case here for every new migration.
 */
class MigrationTest {

    private val dbName = "migration-test.db"

    @get:Rule
    val helper = MigrationTestHelper(
        InstrumentationRegistry.getInstrumentation(),
        ForgeDatabase::class.java,
        emptyList(),
        FrameworkSQLiteOpenHelperFactory()
    )

    @Test
    fun migrate12To13_addsExerciseIdIndex() {
        // Create the v12 schema, then migrate to v13 and validate it matches the v13 schema.
        helper.createDatabase(dbName, 12).close()
        val db = helper.runMigrationsAndValidate(dbName, 13, true, MIGRATION_12_13)

        val cursor = db.query(
            "SELECT name FROM sqlite_master WHERE type='index' " +
                "AND name='index_logged_exercise_exercise_id'"
        )
        cursor.use { assertEquals("exercise_id index should exist after 12→13", 1, it.count) }
    }

    @Test
    fun migrate13To14_addsProgramTables() {
        // Create the v13 schema, then migrate to v14 and validate it matches the v14 schema.
        helper.createDatabase(dbName, 13).close()
        val db = helper.runMigrationsAndValidate(dbName, 14, true, MIGRATION_13_14)

        val cursor = db.query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('program_day','program_slot')"
        )
        cursor.use { assertEquals("program tables should exist after 13→14", 2, it.count) }
    }

    @Test
    fun migrate14To15_dedupesAndAddsBodyweightDateIndex() {
        // Seed two bodyweight rows for the SAME day at v14 (no unique constraint yet).
        helper.createDatabase(dbName, 14).apply {
            execSQL(
                "INSERT INTO bodyweight_entry (id, date_key, weight_lb, recorded_at) " +
                    "VALUES (1, '2026-01-01', 180.0, 1000), (2, '2026-01-01', 181.0, 2000)"
            )
            close()
        }
        val db = helper.runMigrationsAndValidate(dbName, 15, true, MIGRATION_14_15)

        // The unique index exists, and only the most recent same-day row survived.
        db.query(
            "SELECT name FROM sqlite_master WHERE type='index' AND name='index_bodyweight_entry_date_key'"
        ).use { assertEquals("unique date_key index should exist after 14→15", 1, it.count) }

        db.query("SELECT id FROM bodyweight_entry WHERE date_key = '2026-01-01'").use {
            assertEquals("only one row should remain for the day", 1, it.count)
            it.moveToFirst()
            assertEquals("the most recently inserted row should survive", 2L, it.getLong(0))
        }
    }

    @Test
    fun migrate15To16_addsAdaptationEngineTables() {
        helper.createDatabase(dbName, 15).close()
        val db = helper.runMigrationsAndValidate(dbName, 16, true, MIGRATION_15_16)

        db.query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('rest_event','advice_event')"
        ).use { assertEquals("adaptation tables should exist after 15→16", 2, it.count) }

        db.query(
            "SELECT name FROM sqlite_master WHERE type='index' " +
                "AND name IN ('index_rest_event_session_id','index_rest_event_exercise_id','index_advice_event_advice_id')"
        ).use { assertEquals("adaptation indices should exist after 15→16", 3, it.count) }
    }

    @Test
    fun migrate17To18_addsSuggestionOutcomeTable() {
        helper.createDatabase(dbName, 17).close()
        val db = helper.runMigrationsAndValidate(dbName, 18, true, MIGRATION_17_18)

        db.query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name = 'suggestion_outcome'"
        ).use { assertEquals("suggestion_outcome should exist after 17→18", 1, it.count) }
    }

    @Test
    fun migrate19To20_addsSessionSegmentTableAndActiveSeconds() {
        helper.createDatabase(dbName, 19).close()
        val db = helper.runMigrationsAndValidate(dbName, 20, true, MIGRATION_19_20)

        db.query("SELECT name FROM sqlite_master WHERE type='table' AND name = 'session_segment'")
            .use { assertEquals("session_segment should exist after 19→20", 1, it.count) }
        db.query("PRAGMA table_info(`session`)").use { cursor ->
            val cols = mutableSetOf<String>()
            while (cursor.moveToNext()) cols += cursor.getString(cursor.getColumnIndexOrThrow("name"))
            assertEquals("session.active_seconds should exist after 19→20", true, "active_seconds" in cols)
        }
    }

    @Test
    fun migrate18To19_addsDecisionLifecycleColumns() {
        helper.createDatabase(dbName, 18).close()
        val db = helper.runMigrationsAndValidate(dbName, 19, true, MIGRATION_18_19)

        db.query("PRAGMA table_info(`coach_decision`)").use { cursor ->
            val cols = mutableSetOf<String>()
            while (cursor.moveToNext()) cols += cursor.getString(cursor.getColumnIndexOrThrow("name"))
            listOf("day_key", "payload", "applied_at", "outcome", "undo_data").forEach {
                assertEquals("column $it should exist after 18→19", true, it in cols)
            }
        }
    }

    @Test
    fun migrate20To21_addsOverlaySourceColumns() {
        helper.createDatabase(dbName, 20).close()
        val db = helper.runMigrationsAndValidate(dbName, 21, true, MIGRATION_20_21)

        listOf("program_customization", "exercise_customization").forEach { table ->
            db.query("PRAGMA table_info(`$table`)").use { cursor ->
                val cols = mutableSetOf<String>()
                while (cursor.moveToNext()) cols += cursor.getString(cursor.getColumnIndexOrThrow("name"))
                assertEquals("$table.source should exist after 20→21", true, "source" in cols)
            }
        }
    }

    @Test
    fun migrate21To22_addsSwapReattributionColumns() {
        helper.createDatabase(dbName, 21).close()
        val db = helper.runMigrationsAndValidate(dbName, 22, true, MIGRATION_21_22)

        db.query("PRAGMA table_info(`logged_exercise`)").use { cursor ->
            val cols = mutableSetOf<String>()
            while (cursor.moveToNext()) cols += cursor.getString(cursor.getColumnIndexOrThrow("name"))
            assertEquals("logged_exercise.slot_id should exist after 21→22", true, "slot_id" in cols)
        }
        db.query("PRAGMA table_info(`exercise_customization`)").use { cursor ->
            val cols = mutableSetOf<String>()
            while (cursor.moveToNext()) cols += cursor.getString(cursor.getColumnIndexOrThrow("name"))
            assertEquals("exercise_customization.swapped_exercise_id should exist after 21→22", true, "swapped_exercise_id" in cols)
        }
    }

    @Test
    fun migrateFullChain12To22_runsEveryStepInOrder() {
        // The pairwise tests above each validate one hop. This runs the WHOLE locked chain in a
        // single pass — a real v12 install upgrading straight to today's schema — so a gap or an
        // out-of-order/incompatible step between any two versions is caught, not just each hop alone.
        helper.createDatabase(dbName, 12).close()
        helper.runMigrationsAndValidate(dbName, 22, true, *ALL_MIGRATIONS)
    }

    @Test
    fun migrate16To17_addsCoachTables() {
        helper.createDatabase(dbName, 16).close()
        val db = helper.runMigrationsAndValidate(dbName, 17, true, MIGRATION_16_17)

        db.query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('coach_pass','coach_decision')"
        ).use { assertEquals("coach tables should exist after 16→17", 2, it.count) }

        db.query(
            "SELECT name FROM sqlite_master WHERE type='index' AND name = 'index_coach_decision_week_id'"
        ).use { assertEquals("coach decision index should exist after 16→17", 1, it.count) }
    }
}
