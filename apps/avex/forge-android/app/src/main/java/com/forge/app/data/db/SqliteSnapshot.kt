package com.forge.app.data.db

import android.content.ContentValues
import android.database.Cursor
import android.database.sqlite.SQLiteDatabase
import androidx.sqlite.db.SupportSQLiteDatabase
import java.io.File

/**
 * Backup for SQLite versions without VACUUM INTO. Read through SQLite, never copy a live base
 * file: committed WAL frames and concurrent writers must participate in the snapshot.
 *
 * The source transaction pins one connection/snapshot and excludes writers until the copy ends.
 * Call on a worker thread. The separate destination has no readers and uses a rollback journal.
 */
internal fun copySqliteSnapshot(source: SupportSQLiteDatabase, destination: File) {
    source.beginTransactionNonExclusive()
    try {
        SQLiteDatabase.openOrCreateDatabase(destination, null).use { target ->
            target.beginTransaction()
            try {
                val schema = source.query(
                    "SELECT type, name, sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY rowid"
                ).use { cursor ->
                    buildList {
                        while (cursor.moveToNext()) add(Triple(cursor.getString(0), cursor.getString(1), cursor.getString(2)))
                    }
                }
                for ((type, name, sql) in schema) if (type == "table") {
                    // Android creates this table when opening a new destination.
                    if (name == "android_metadata") target.execSQL("DELETE FROM android_metadata")
                    else target.execSQL(sql)
                }
                fun copyRows(table: String) {
                    val quoted = "\"${table.replace("\"", "\"\"")}\""
                    source.query("SELECT * FROM $quoted").use { cursor ->
                        while (cursor.moveToNext()) {
                            val values = ContentValues(cursor.columnCount)
                            for (i in 0 until cursor.columnCount) {
                                val name = cursor.getColumnName(i)
                                when (cursor.getType(i)) {
                                    Cursor.FIELD_TYPE_NULL -> values.putNull(name)
                                    Cursor.FIELD_TYPE_INTEGER -> values.put(name, cursor.getLong(i))
                                    Cursor.FIELD_TYPE_FLOAT -> values.put(name, cursor.getDouble(i))
                                    Cursor.FIELD_TYPE_BLOB -> values.put(name, cursor.getBlob(i))
                                    else -> values.put(name, cursor.getString(i))
                                }
                            }
                            target.insertOrThrow(quoted, null, values)
                        }
                    }
                }
                for ((type, name, _) in schema) if (type == "table") copyRows(name)
                // AUTOINCREMENT can remain above max(id) after deletes; preserve that history too.
                val hasSequence = source.query("SELECT 1 FROM sqlite_master WHERE name='sqlite_sequence'")
                    .use { it.moveToFirst() }
                if (hasSequence) {
                    target.execSQL("DELETE FROM sqlite_sequence")
                    copyRows("sqlite_sequence")
                }
                for ((type, _, sql) in schema) if (type != "table") target.execSQL(sql)
                for (pragma in listOf("user_version", "application_id")) {
                    val value = source.query("PRAGMA $pragma").use { it.moveToFirst(); it.getInt(0) }
                    target.execSQL("PRAGMA $pragma=$value")
                }
                target.setTransactionSuccessful()
            } finally { target.endTransaction() }
        }
    } finally { source.endTransaction() }
}
