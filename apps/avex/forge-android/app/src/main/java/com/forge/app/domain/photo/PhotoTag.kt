package com.forge.app.domain.photo

import com.forge.app.program.MuscleGroup

/**
 * The two tag vocabularies a progress photo carries beyond its [PhotoPose] angle.
 *
 * **Muscle tags** are the app's own [MuscleGroup] values, stored as their `code`. They are a
 * separate axis from the pose on purpose: the pose says where the camera stood, the muscle tags say
 * what the shot documents, and one back shot legitimately covers Back and Rear Delts. Reusing the
 * program's enum rather than inventing a photo-only list is what lets a shot be read against the
 * training that produced it, instead of against a second, drifting spelling of the same body.
 *
 * **Free tags** are whatever the user invents ("fasted", "week-12", "cut"). They are normalized to
 * one canonical spelling on the way in, so "Week 12", "#week 12" and "week-12" are one tag and the
 * filter rail does not fill with near-duplicates of the same idea.
 */
object PhotoTag {

    /** Long enough for "post-vacation", short enough that a chip never wraps its own rail. */
    const val MAX_LENGTH = 24

    /** More than this on one photo stops being a label and starts being a note. */
    const val MAX_PER_PHOTO = 8

    private val ALLOWED = Regex("""[^a-z0-9\-]""")

    /**
     * Fold [raw] to its canonical tag spelling, or null when nothing usable is left. Lower-cased, a
     * leading hash dropped, runs of spaces and punctuation collapsed to single hyphens, and trimmed
     * of stray hyphens at either end.
     */
    fun normalize(raw: String): String? {
        val folded = raw.trim().removePrefix("#").lowercase()
            .replace(Regex("""\s+"""), "-")
            .replace(ALLOWED, "")
            .replace(Regex("-{2,}"), "-")
            .trim('-')
            .take(MAX_LENGTH)
            .trim('-')
        return folded.ifBlank { null }
    }

    /** How a tag renders anywhere the user sees it. */
    fun display(tag: String): String = "#$tag"

    /** Add [raw] to [current] (normalized, deduped, capped); returns [current] when it adds nothing. */
    fun added(current: List<String>, raw: String): List<String> {
        val tag = normalize(raw) ?: return current
        if (tag in current || current.size >= MAX_PER_PHOTO) return current
        return current + tag
    }
}

/** Resolve stored muscle codes to their enum values, in enum order, dropping anything unknown. */
fun musclesFromCodes(codes: List<String>): List<MuscleGroup> =
    MuscleGroup.entries.filter { it.code in codes }

/** The muscle groups actually used across [all], in enum order — the muscle filter rail's contents. */
fun musclesPresent(all: List<List<String>>): List<MuscleGroup> {
    val used = all.flatten().toSet()
    return MuscleGroup.entries.filter { it.code in used }
}
