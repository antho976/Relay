package com.tally.app.data

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.tally.app.data.db.TallyDatabase
import com.tally.core.GoalKind
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDate

/**
 * A ledger written by the first release must open in this one with nothing lost. The version 1
 * database is built from its exported schema, exactly as that release created it, filled, and
 * then opened by Room at the current version: the auto-migration runs and Room checks the result
 * against the current schema, so a missing default or a wrong column fails here, not on a phone.
 */
@RunWith(RobolectricTestRunner::class)
class MigrationTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val name = "migration-test.db"
    private var opened: TallyDatabase? = null

    @After fun clean() {
        opened?.close()
        context.deleteDatabase(name)
    }

    /** The version 1 tables, indices and Room's own bookkeeping, from the exported schema. */
    private fun version1Statements(): List<String> {
        val text = checkNotNull(javaClass.classLoader?.getResourceAsStream("com.tally.app.data.db.TallyDatabase/1.json")) {
            "The version 1 schema is not on the test classpath"
        }.bufferedReader().use { it.readText() }
        val db = JSONObject(text).getJSONObject("database")
        val out = ArrayList<String>()
        val entities = db.getJSONArray("entities")
        for (i in 0 until entities.length()) {
            val e = entities.getJSONObject(i)
            val table = e.getString("tableName")
            out += e.getString("createSql").replace("\${TABLE_NAME}", table)
            val indices = e.optJSONArray("indices") ?: continue
            for (k in 0 until indices.length()) {
                val index = indices.getJSONObject(k)
                out += index.getString("createSql").replace("\${TABLE_NAME}", table).replace("\${INDEX_NAME}", index.getString("name"))
            }
        }
        val setup = db.getJSONArray("setupQueries")
        for (i in 0 until setup.length()) out += setup.getString(i)
        return out
    }

    @Test fun aVersion1LedgerOpensWithEverythingInIt() = runTest {
        val file = context.getDatabasePath(name)
        file.parentFile?.mkdirs()
        val day = LocalDate.of(2026, 9, 14).toEpochDay()
        SQLiteDatabase.openOrCreateDatabase(file, null).use { v1 ->
            version1Statements().forEach { v1.execSQL(it) }
            v1.execSQL("INSERT INTO accounts (id, name, type, openingBalance, archived, sortOrder) VALUES (1, 'Chequing', 'CHEQUING', 100000, 0, 0)")
            v1.execSQL("INSERT INTO categories (id, name, kind, color, icon, archived, sortOrder) VALUES (1, 'Groceries', 'EXPENSE', 0, 'cart', 0, 0)")
            v1.execSQL(
                "INSERT INTO transactions (id, type, amount, date, accountId, toAccountId, categoryId, note, recurringId, createdAt) " +
                    "VALUES (1, 'EXPENSE', 6842, $day, 1, NULL, 1, 'Metro', NULL, 0)"
            )
            v1.execSQL("INSERT INTO goals (id, name, target, targetDate, color, archived) VALUES (1, 'Lisbon in May', 320000, NULL, 1, 0)")
            v1.execSQL("INSERT INTO goal_contributions (id, goalId, amount, date, note) VALUES (1, 1, 50000, $day, '')")
            v1.version = 1
        }

        val db = Room.databaseBuilder(context, TallyDatabase::class.java, name).allowMainThreadQueries().build()
        opened = db

        val goal = db.goals().all().single()
        assertEquals("Lisbon in May", goal.name)
        assertEquals("A goal from before kinds is a savings pot", GoalKind.SAVINGS, goal.kind)
        assertNull(goal.accountId)
        assertEquals(0, goal.percent)
        assertNull(goal.startDate)
        assertEquals(0L, goal.startAmount)
        assertEquals(1, db.goals().allContributions().size)

        val balance = db.accounts().observeBalances().first().single()
        assertEquals(100_000L - 6_842, balance.balance)
        assertNull("No values were recorded before version 2", balance.valuedOn)
        assertTrue(db.values().all().isEmpty())
    }
}
