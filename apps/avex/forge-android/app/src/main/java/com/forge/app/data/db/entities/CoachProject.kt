package com.forge.app.data.db.entities

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * One named improvement project (Coach v3 D) — the visible answer to "what should I improve?".
 *
 * ONE runs at a time, on purpose. A list of things to fix is a list nobody acts on; a single
 * project with a why, a plan and a finish line is a thing that gets done. Completed and abandoned
 * projects stay in the table: the record of what the coach hunted, and whether it worked, IS the
 * roadmap the user sees.
 */
@Entity(tableName = "coach_project")
data class CoachProject(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** A [com.forge.app.domain.coach.ProjectScanner.Kind] code. */
    @ColumnInfo(name = "kind") val kind: String,
    /** The project's name, in the coach's voice ("Bring up your rear delts"). */
    @ColumnInfo(name = "name") val name: String,
    @ColumnInfo(name = "why") val why: String,
    @ColumnInfo(name = "plan") val plan: String,
    /** What ends it — a project without one is a complaint. */
    @ColumnInfo(name = "finish_line") val finishLine: String,
    /** Muscle code, exercise id or ratio name this project is about; empty for whole-athlete work. */
    @ColumnInfo(name = "target_key") val targetKey: String = "",
    @ColumnInfo(name = "weeks") val weeks: Int = 4,
    @ColumnInfo(name = "started_at") val startedAt: Long,
    /** Reached its finish line. */
    @ColumnInfo(name = "completed_at") val completedAt: Long? = null,
    /** Dropped, by the user or by the coach when the lever stopped being the biggest one. */
    @ColumnInfo(name = "abandoned_at") val abandonedAt: Long? = null
) {
    val isActive: Boolean get() = completedAt == null && abandonedAt == null
}
