package com.forge.app.data.db

import androidx.room.Database
import androidx.room.RoomDatabase
import com.forge.app.data.db.dao.AppMetaDao
import com.forge.app.data.db.entities.AppMeta

@Database(
    entities = [AppMeta::class],
    version = 1,
    exportSchema = true
)
abstract class ForgeDatabase : RoomDatabase() {
    abstract fun appMetaDao(): AppMetaDao
}
