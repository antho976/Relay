package com.forge.app.data.db.entities

import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Phase 0 placeholder entity so Room has a schema to compile against. Real entities
 * (Session, LoggedExercise, LoggedSet, etc.) land in Phase 2.
 *
 * Keep this table around long-term: a single-row meta table is a handy place for
 * schema-version stamps, migration markers, and one-off feature flags.
 */
@Entity(tableName = "app_meta")
data class AppMeta(
    @PrimaryKey val id: Int = 1,
    val schemaVersion: Int = 1,
    val installedAt: Long
)
