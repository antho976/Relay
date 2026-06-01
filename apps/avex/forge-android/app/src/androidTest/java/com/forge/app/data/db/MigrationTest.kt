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
}
