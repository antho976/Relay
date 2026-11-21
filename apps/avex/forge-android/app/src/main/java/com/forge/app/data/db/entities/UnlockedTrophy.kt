package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * One row per trophy id that has been unlocked. The trophy *definition* lives in
 * [com.forge.app.program.Trophies] (static catalogue); this table only records the
 * unlock event so we don't re-fire the animation.
 */
@Entity(tableName = "unlocked_trophy")
data class UnlockedTrophy(
    @PrimaryKey @ColumnInfo(name = "trophy_id") val trophyId: String,
    @ColumnInfo(name = "unlocked_at") val unlockedAt: Long
)
