package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * One day of the **active program** — the data-driven replacement for the hardcoded
 * `Program.days` (program-unlock plan, Phase 0). The whole program is regenerated wholesale
 * (generate / rotate), and history is keyed elsewhere, so these rows carry no stable identity
 * obligation — they can be cleared and rewritten freely.
 *
 * [archetype] is the split-template slot this day came from (e.g. "push", "upper"); [word] is
 * the rotated spine word and [accentHex] the day's identity colour (mirrors `DayPlan`).
 */
@Entity(tableName = "program_day")
data class ProgramDay(
    @PrimaryKey @ColumnInfo(name = "id") val id: String,
    @ColumnInfo(name = "position") val position: Int,
    @ColumnInfo(name = "name") val name: String,
    @ColumnInfo(name = "word") val word: String,
    @ColumnInfo(name = "accent_hex") val accentHex: String,
    @ColumnInfo(name = "archetype") val archetype: String
)
