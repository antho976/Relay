package com.forge.app.data.repo

import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session

/** Version 1 training-history set contract, shared by full, weekly and session JSON writers. */
internal fun exportSetFields(set: LoggedSet): Map<String, Any> = buildMap {
    put("weightText", set.weightText)
    set.weightLb?.let { put("weightLb", it) }
    put("reps", set.reps)
    set.rpe?.let { put("rpe", it) }
    put("completedAt", set.completedAt)
    put("difficultyTag", set.difficultyTag.orEmpty())
    put("durationSeconds", set.durationSeconds ?: 0)
    put("isAssisted", set.isAssisted)
    put("isAmrap", set.isAmrap)
    put("toFailure", set.toFailure)
    put("setType", set.setType.orEmpty())
    put("dropAnnotation", set.dropAnnotation.orEmpty())
}

/**
 * Session fields shared by the full, weekly and session JSON writers, and read back by
 * `ForgeJsonImporter`. The three used to be written by hand and had drifted: the weekly and session
 * files dropped `isUntracked` (and the weekly one `sessionType`), and the importer defaults a
 * missing `isUntracked` to false, so an excluded session came back counted.
 */
internal fun exportSessionFields(s: Session, activeSeconds: Int, mood: String): Map<String, Any> = buildMap {
    put("id", s.id)
    put("dayKey", s.dayKey)
    put("startedAt", s.startedAt)
    put("finishedAt", s.finishedAt ?: 0L)
    put("activeSeconds", activeSeconds)
    put("totalVolumeLb", s.totalVolumeLb ?: 0.0)
    put("prCount", s.prCount)
    put("setCount", s.setCount)
    put("sessionType", s.sessionType)
    put("intensity", s.intensity)
    put("isUntracked", s.isUntracked)
    put("tags", s.tags)
    put("journal", s.journal)
    put("mood", mood)
}

/**
 * Exercise fields shared by the three JSON writers. `wasPr`, `hitFullTarget` and `supersetGroup`
 * were missing from all of them, so after an import `was_pr` was 0 everywhere: recent PRs, the PR
 * count behind trophies and milestones, and PR session dates all read zero while each session's own
 * PR count still said N.
 */
internal fun exportExerciseFields(ex: LoggedExercise): Map<String, Any> = buildMap {
    put("exerciseId", ex.exerciseId)
    // The DISPLAY name: the raw seed-split ids ("ua1") resolve only on the display path, and a
    // reader matching on the raw id re-imported years of bench press as a movement called "Ua1".
    put("name", com.forge.app.program.Program.exerciseDisplayName(ex.exerciseId, ex.swappedName))
    put("swappedName", ex.swappedName.orEmpty())
    put("orderIndex", ex.orderIndex)
    put("difficulty", ex.difficulty?.name.orEmpty())
    put("skipped", ex.skipped)
    put("note", ex.note.orEmpty())
    put("wasPr", ex.wasPr)
    put("hitFullTarget", ex.hitFullTarget)
    ex.supersetGroup?.let { put("supersetGroup", it) }
}

/**
 * Cardio fields shared by the full and weekly JSON writers. `date` is the entry's epoch instant:
 * the weekly file wrote a `yyyy-MM-dd` string, so an 08:00 and an 18:00 run of equal length both
 * re-imported at midnight and the second was dropped as the first's duplicate. A nullable value is
 * omitted, never written as "" (which reads back as a default rather than as absent), and never as
 * JSONObject.NULL (which `optString` reads back as the text "null"). `intervalCount`, `hrZone` and `conditions`
 * were missing from both, though `intervalCount` feeds conditioning load and an Academy unlock.
 */
internal fun exportCardioFields(c: CardioEntry): Map<String, Any> = buildMap {
    put("date", c.date)
    put("type", c.type)
    put("durationMin", c.durationMin)
    c.distanceKm?.let { put("distanceKm", it) }
    put("effort", c.effort.orEmpty())
    put("restReason", c.restReason.orEmpty())
    put("note", c.note.orEmpty())
    c.intervalCount?.let { put("intervalCount", it) }
    c.hrZone?.let { put("hrZone", it) }
    // Per-type fields (GYMAP-38); elevation stays canonical metres like distance is km.
    c.inclinePct?.let { put("inclinePct", it) }
    c.laps?.let { put("laps", it) }
    c.elevationM?.let { put("elevationM", it) }
    c.conditions?.let { put("conditions", it) }
}
