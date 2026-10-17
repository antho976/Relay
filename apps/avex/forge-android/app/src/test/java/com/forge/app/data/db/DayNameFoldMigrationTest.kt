package com.forge.app.data.db

import android.app.Application
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Migration 39→40 folds the legacy day-name overrides into the program's day names, once. */
@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [34])
class DayNameFoldMigrationTest {

    private val db = inMemoryForgeDb()
    private val sql = db.openHelper.writableDatabase

    @After fun close() = db.close()

    private fun day(id: String, name: String, position: Int) = sql.execSQL(
        "INSERT INTO program_day (id, position, name, word, accent_hex, archetype) VALUES (?, ?, ?, 'PUSH', '#FFFFFF', ?)",
        arrayOf(id, position, name, id)
    )

    private fun override(dayKey: String, name: String) =
        sql.execSQL("INSERT INTO day_name_override (day_key, custom_name) VALUES (?, ?)", arrayOf(dayKey, name))

    private fun names(): Map<String, String> =
        sql.query("SELECT id, name FROM program_day").use { c ->
            buildMap { while (c.moveToNext()) put(c.getString(0), c.getString(1)) }
        }

    @Test
    fun overridesBecomeTheProgramsDayNamesAndTheTableEmpties() {
        day("upper-a", "Upper A", 0)
        day("lower-a", "Lower A", 1)
        day("upper-b", "Upper B", 2)
        override("upper-a", "  Chest day ")
        // A blank override never showed as a name; it is dropped, not folded.
        override("lower-a", "   ")
        // A day the program no longer has: nothing to fold into.
        override("gone-day", "Old day")

        MIGRATION_39_40.migrate(sql)

        assertEquals(mapOf("upper-a" to "Chest day", "lower-a" to "Lower A", "upper-b" to "Upper B"), names())
        sql.query("SELECT COUNT(*) FROM day_name_override").use { it.moveToFirst(); assertEquals(0, it.getInt(0)) }
    }
}
