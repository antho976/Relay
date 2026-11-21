package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * Records near-miss trophy moments — e.g. "9/10 PRs, On a Roll missed" — so the user
 * can see how close they came on the Trophies screen (#136).
 */
@Entity(tableName = "trophy_near_miss")
data class TrophyNearMiss(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "trophy_id") val trophyId: String,
    @ColumnInfo(name = "trophy_name") val trophyName: String,
    @ColumnInfo(name = "progress") val progress: Int,
    @ColumnInfo(name = "target") val target: Int,
    @ColumnInfo(name = "recorded_at") val recordedAt: Long
)
