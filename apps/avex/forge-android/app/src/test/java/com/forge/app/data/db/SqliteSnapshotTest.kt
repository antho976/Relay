package com.forge.app.data.db

import android.app.Application
import android.content.Context
import android.database.sqlite.SQLiteDatabase
import androidx.sqlite.db.SupportSQLiteDatabase
import androidx.sqlite.db.SupportSQLiteOpenHelper
import androidx.sqlite.db.framework.FrameworkSQLiteOpenHelperFactory
import androidx.test.core.app.ApplicationProvider
import com.forge.app.RestoreApply
import com.forge.app.RestoreManifest
import java.io.File
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [34])
class SqliteSnapshotTest {
    @get:Rule val temp = TemporaryFolder()
    private fun helper(): SupportSQLiteOpenHelper {
        val context = ApplicationProvider.getApplicationContext<Context>()
        return FrameworkSQLiteOpenHelperFactory().create(SupportSQLiteOpenHelper.Configuration.builder(context)
            .name(File(temp.root, "source.db").path)
            .callback(object : SupportSQLiteOpenHelper.Callback(7) {
                override fun onCreate(db: SupportSQLiteDatabase) {
                    db.execSQL("CREATE TABLE entries (id INTEGER PRIMARY KEY AUTOINCREMENT, label TEXT, amount REAL, payload BLOB)")
                    db.execSQL("CREATE INDEX entries_label ON entries(label)")
                }
                override fun onUpgrade(db: SupportSQLiteDatabase, oldVersion: Int, newVersion: Int) = Unit
            }).build()).also { it.setWriteAheadLoggingEnabled(true) }
    }

    @Test fun `logical snapshot preserves values schema version and deleted autoincrement high water`() {
        helper().use { helper ->
            val source = helper.writableDatabase
            source.execSQL("INSERT INTO entries(id,label,amount,payload) VALUES(1,?,2.5,?)", arrayOf("quoted ' value", byteArrayOf(1,2,3)))
            source.execSQL("INSERT INTO entries(id) VALUES(100)")
            source.execSQL("DELETE FROM entries WHERE id=100")
            val target = File(temp.root, "copy.db")
            copySqliteSnapshot(source, target)
            SQLiteDatabase.openDatabase(target.path, null, SQLiteDatabase.OPEN_READWRITE).use { copy ->
                assertEquals(7, copy.version)
                copy.rawQuery("SELECT label,amount,payload FROM entries", null).use {
                    assertTrue(it.moveToFirst());assertEquals("quoted ' value",it.getString(0))
                    assertEquals(2.5,it.getDouble(1),0.0);assertArrayEquals(byteArrayOf(1,2,3),it.getBlob(2))
                }
                copy.execSQL("INSERT INTO entries(label) VALUES('next')")
                copy.rawQuery("SELECT max(id) FROM entries",null).use { it.moveToFirst();assertEquals(101,it.getInt(0)) }
                copy.rawQuery("PRAGMA integrity_check",null).use { it.moveToFirst();assertEquals("ok",it.getString(0)) }
                copy.rawQuery("SELECT name FROM sqlite_master WHERE name='entries_label'",null).use { assertTrue(it.moveToFirst()) }
            }
        }
    }

    @Test fun `restore rollback preserves committed rows that existed only in original WAL`() {
        val files = temp.newFolder("files")
        val live = File(temp.newFolder("live"),"forge.db")
        helper().use { helper ->
            val source = helper.writableDatabase
            source.query("PRAGMA wal_autocheckpoint=0").use { it.moveToFirst() }
            source.execSQL("INSERT INTO entries(label) VALUES('committed')")
            val sourceFile = File(source.path!!)
            assertTrue(File(sourceFile.path+"-wal").length()>0)
            // Copy the coherent idle database/WAL pair to model a process exiting without close.
            sourceFile.copyTo(live)
            File(sourceFile.path+"-wal").copyTo(File(live.path+"-wal"))
        }
        File(files,"pending_restore.db").writeText("deliberately unopenable incoming database")
        assertTrue(RestoreManifest.publish(files))
        assertTrue(RestoreApply.apply(files,live))
        assertTrue(RestoreApply.revert(files,live))
        SQLiteDatabase.openDatabase(live.path,null,SQLiteDatabase.OPEN_READWRITE).use { original ->
            original.rawQuery("SELECT label FROM entries",null).use {
                assertTrue(it.moveToFirst());assertEquals("committed",it.getString(0))
            }
            original.rawQuery("PRAGMA integrity_check",null).use { it.moveToFirst();assertEquals("ok",it.getString(0)) }
        }
    }
}
