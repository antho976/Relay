package com.forge.app.data.repo

import com.forge.app.data.db.entities.LoggedSet

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
