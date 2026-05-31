package com.forge.app.data.db

import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase

/**
 * Real Room migrations. The schema is **locked from v12 onward** — every change from here
 * needs a Migration object here (and a bumped [ForgeDatabase] version). Versions ≤11 predate
 * the lock and still reset destructively (see [com.forge.app.di.DatabaseModule]); v12+ preserve
 * data. If a future version bump ships without a matching migration, Room throws loudly at
 * startup — that's intentional, and far better than silently wiping the user's history.
 *
 * Each migration's SQL must produce exactly the schema Room expects for the target version
 * (validated against `app/schemas/...<version>.json`).
 */

/** v12 → v13: add the index on `logged_exercise.exercise_id` (day-screen + stats hot path). */
val MIGRATION_12_13 = object : Migration(12, 13) {
    override fun migrate(db: SupportSQLiteDatabase) {
        db.execSQL(
            "CREATE INDEX IF NOT EXISTS `index_logged_exercise_exercise_id` " +
                "ON `logged_exercise` (`exercise_id`)"
        )
    }
}

/** All migrations, in order. Register every new one here. */
val ALL_MIGRATIONS: Array<Migration> = arrayOf(
    MIGRATION_12_13
)
