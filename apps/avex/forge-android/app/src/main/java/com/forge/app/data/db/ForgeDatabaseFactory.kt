package com.forge.app.data.db

import android.content.Context
import androidx.room.Room
import androidx.room.RoomDatabase
import com.forge.app.BuildConfig

/**
 * The one Room configuration for [ForgeDatabase], shared by the production provider
 * (`DatabaseModule.provideDatabase`) and by restore validation (`BackupRepository`).
 *
 * Restore used to accept any SQLite file that carried `user_version` and three table NAMES, then
 * discovered at the next boot — after the live database had been replaced and its snapshot
 * discarded — that Room could not open it. Validating a candidate through this exact builder,
 * migrations and identity check included, moves that discovery to the restore screen, where
 * refusing still costs nothing. Two builders would let the two drift; one cannot.
 *
 * @param name a database name (resolved under `databases/`) or an absolute path, which the
 *   framework's open helper uses as-is — that is how a staged file is opened where it lies.
 */
fun forgeDatabaseBuilder(context: Context, name: String): RoomDatabase.Builder<ForgeDatabase> =
    Room.databaseBuilder(context, ForgeDatabase::class.java, name)
        // Schema is LOCKED from v12 onward — real migrations preserve data (see Migrations.kt).
        .addMigrations(*ALL_MIGRATIONS)
        // Only the pre-lock versions (≤11) may still reset rather than crash. Future bumps
        // without a migration will fail loudly at startup instead of silently wiping data.
        .fallbackToDestructiveMigrationFrom(true, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11)
        .apply {
            // Debug only: parallel feature branches sit at different schema versions, so
            // sideloading an older-schema build over a newer on-disk DB opens as a DOWNGRADE
            // (e.g. installed v31, then a v29 branch) — a path Room can't satisfy, and it
            // crashes at startup. In dev that data is disposable, so recreate destructively
            // instead. Release deliberately keeps the loud crash: a real downgrade can only
            // come from a shipped version rollback, and we'd rather fail than silently wipe a
            // user's history (matches the "never silently wipe data" lock above).
            if (BuildConfig.DEBUG) fallbackToDestructiveMigrationOnDowngrade(true)
        }
