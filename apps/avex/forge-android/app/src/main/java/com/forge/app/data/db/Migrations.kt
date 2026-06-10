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

/** v13 → v14: add the data-driven program tables (program-unlock). New empty tables — additive, no existing data touched. */
val MIGRATION_13_14 = object : Migration(13, 14) {
    override fun migrate(db: SupportSQLiteDatabase) {
        db.execSQL(
            "CREATE TABLE IF NOT EXISTS `program_day` (`id` TEXT NOT NULL, `position` INTEGER NOT NULL, " +
                "`name` TEXT NOT NULL, `word` TEXT NOT NULL, `accent_hex` TEXT NOT NULL, " +
                "`archetype` TEXT NOT NULL, PRIMARY KEY(`id`))"
        )
        db.execSQL(
            "CREATE TABLE IF NOT EXISTS `program_slot` (`id` TEXT NOT NULL, `day_id` TEXT NOT NULL, " +
                "`position` INTEGER NOT NULL, `exercise_lib_id` TEXT NOT NULL, `sets` INTEGER NOT NULL, " +
                "`reps` TEXT NOT NULL, PRIMARY KEY(`id`))"
        )
        db.execSQL(
            "CREATE INDEX IF NOT EXISTS `index_program_slot_day_id` ON `program_slot` (`day_id`)"
        )
    }
}

/**
 * v14 → v15: enforce one bodyweight entry per day. De-duplicates any pre-existing same-day rows
 * (keeping the most recently inserted), then adds the unique index so REPLACE actually upserts by
 * date instead of appending a new row.
 */
val MIGRATION_14_15 = object : Migration(14, 15) {
    override fun migrate(db: SupportSQLiteDatabase) {
        db.execSQL(
            "DELETE FROM `bodyweight_entry` WHERE `id` NOT IN " +
                "(SELECT MAX(`id`) FROM `bodyweight_entry` GROUP BY `date_key`)"
        )
        db.execSQL(
            "CREATE UNIQUE INDEX IF NOT EXISTS `index_bodyweight_entry_date_key` " +
                "ON `bodyweight_entry` (`date_key`)"
        )
    }
}

/**
 * v15 → v16: adaptation-engine tables. `rest_event` records realized rest between sets
 * (planned vs actual, how the rest ended) — RestAdvisor's tuning signal. `advice_event`
 * logs shown/applied/dismissed per recommendation id for cooldowns + feedback. New empty
 * tables — additive, no existing data touched.
 */
val MIGRATION_15_16 = object : Migration(15, 16) {
    override fun migrate(db: SupportSQLiteDatabase) {
        db.execSQL(
            "CREATE TABLE IF NOT EXISTS `rest_event` (" +
                "`id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, " +
                "`session_id` INTEGER NOT NULL, " +
                "`exercise_id` TEXT NOT NULL, " +
                "`set_index` INTEGER NOT NULL, " +
                "`planned_seconds` INTEGER NOT NULL, " +
                "`realized_seconds` INTEGER NOT NULL, " +
                "`ended_by` TEXT NOT NULL, " +
                "`seconds_added` INTEGER NOT NULL, " +
                "`logged_at` INTEGER NOT NULL, " +
                "FOREIGN KEY(`session_id`) REFERENCES `session`(`id`) " +
                "ON UPDATE NO ACTION ON DELETE CASCADE )"
        )
        db.execSQL("CREATE INDEX IF NOT EXISTS `index_rest_event_session_id` ON `rest_event` (`session_id`)")
        db.execSQL("CREATE INDEX IF NOT EXISTS `index_rest_event_exercise_id` ON `rest_event` (`exercise_id`)")
        db.execSQL(
            "CREATE TABLE IF NOT EXISTS `advice_event` (" +
                "`id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, " +
                "`advice_id` TEXT NOT NULL, " +
                "`action` TEXT NOT NULL, " +
                "`logged_at` INTEGER NOT NULL)"
        )
        db.execSQL("CREATE INDEX IF NOT EXISTS `index_advice_event_advice_id` ON `advice_event` (`advice_id`)")
    }
}

/** All migrations, in order. Register every new one here. */
val ALL_MIGRATIONS: Array<Migration> = arrayOf(
    MIGRATION_12_13,
    MIGRATION_13_14,
    MIGRATION_14_15,
    MIGRATION_15_16
)
