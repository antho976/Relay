package com.forge.app.data.db

import android.app.Application
import android.content.Context
import android.database.sqlite.SQLiteDatabase
import androidx.test.core.app.ApplicationProvider
import com.forge.app.data.db.entities.WearCommand
import java.io.File
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [34])
class WatchCommandMigrationTest {
    @Test fun `version 36 migrates to current Room schema without losing history`() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val path = context.getDatabasePath("watch-migration.db")
        path.parentFile!!.mkdirs()
        val schema = JSONObject(File("schemas/com.forge.app.data.db.ForgeDatabase/36.json").readText())
            .getJSONObject("database")
        SQLiteDatabase.openOrCreateDatabase(path, null).use { sqlite ->
            val entities = schema.getJSONArray("entities")
            for (i in 0 until entities.length()) {
                val entity = entities.getJSONObject(i)
                fun sql(raw: String) = raw.replace("\${TABLE_NAME}", entity.getString("tableName"))
                sqlite.execSQL(sql(entity.getString("createSql")))
                val indices = entity.optJSONArray("indices") ?: org.json.JSONArray()
                for (j in 0 until indices.length()) sqlite.execSQL(sql(indices.getJSONObject(j).getString("createSql")))
            }
            val setup = schema.getJSONArray("setupQueries")
            for (i in 0 until setup.length()) sqlite.execSQL(setup.getString(i))
            sqlite.execSQL("INSERT INTO session VALUES (99, 'saved', 1000, 2000, 50, 0, 1, 0, '', 'normal', 0, 'keep me', 'normal', 1)")
            sqlite.version = 36
        }
        val db = forgeDatabaseBuilder(context, path.path).allowMainThreadQueries().build()
        try {
            assertEquals("keep me", db.sessionDao().get(99)!!.journal)
            db.wearCommandDao().insert(WearCommand("a", "{}", 1000))
            assertNotNull(db.wearCommandDao().get("a"))
        } finally { db.close(); context.deleteDatabase("watch-migration.db") }
    }
}
