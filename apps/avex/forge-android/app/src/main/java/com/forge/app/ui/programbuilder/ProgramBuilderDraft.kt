package com.forge.app.ui.programbuilder

import org.json.JSONArray
import org.json.JSONObject

/**
 * The builder's unsaved state as one JSON string, small enough for `SavedStateHandle`: the edited
 * document (every day, exercise, sets × reps, in order, with the uids the screen keys on), whether it
 * differs from the saved program, and where the user was in the editor (the open day and its open
 * dialog). Written on every edit and read back when the ViewModel is recreated after Android killed
 * the process behind a retained task, so the draft the user was looking at is the draft they return
 * to. Mirrors the hand-rolled `org.json` round-trip of [com.forge.app.ui.gym.freestyle.FreestyleDraft]
 * and [com.forge.app.ui.onboarding.OnboardingDraft].
 */
internal data class ProgramBuilderDraft(
    val days: List<BuilderDay>,
    val dirty: Boolean,
    val openDayUid: String?,
    val dialog: DayDialog
) {
    fun toJson(): String = JSONObject().apply {
        put("schema", SCHEMA)
        put("dirty", dirty)
        openDayUid?.let { put("openDay", it) }
        put("dialog", dialogKind(dialog))
        dialogExercise(dialog)?.let { put("dialogEx", it) }
        put("days", JSONArray(days.map { d ->
            JSONObject().apply {
                put("uid", d.uid)
                put("key", d.key)
                put("name", d.name)
                put("archetype", d.archetype)
                put("accent", d.accentHex)
                if (d.word.isNotEmpty()) put("word", d.word)
                put("exercises", JSONArray(d.exercises.map { e ->
                    JSONObject().apply {
                        put("uid", e.uid)
                        put("libId", e.libId)
                        put("name", e.name)
                        put("muscle", e.muscle)
                        put("sets", e.sets)
                        put("reps", e.reps)
                    }
                }))
            }
        }))
    }.toString()

    companion object {
        /** Bump if the shape changes so a draft written by an older build is dropped, not misread. */
        private const val SCHEMA = 1

        private const val DIALOG_NONE = "none"
        private const val DIALOG_RENAME = "rename"
        private const val DIALOG_ADD = "add"
        private const val DIALOG_SETS = "sets"
        private const val DIALOG_SWAP = "swap"

        private fun dialogKind(dialog: DayDialog): String = when (dialog) {
            DayDialog.None -> DIALOG_NONE
            DayDialog.Rename -> DIALOG_RENAME
            DayDialog.AddExercises -> DIALOG_ADD
            is DayDialog.SetsReps -> DIALOG_SETS
            is DayDialog.Swap -> DIALOG_SWAP
        }

        private fun dialogExercise(dialog: DayDialog): String? = when (dialog) {
            is DayDialog.SetsReps -> dialog.exerciseUid
            is DayDialog.Swap -> dialog.exerciseUid
            else -> null
        }

        private fun dialogOf(kind: String, exerciseUid: String?): DayDialog = when (kind) {
            DIALOG_RENAME -> DayDialog.Rename
            DIALOG_ADD -> DayDialog.AddExercises
            DIALOG_SETS -> exerciseUid?.let { DayDialog.SetsReps(it) } ?: DayDialog.None
            DIALOG_SWAP -> exerciseUid?.let { DayDialog.Swap(it) } ?: DayDialog.None
            else -> DayDialog.None
        }

        /** Null on any parse failure or a stale schema — the builder then loads the saved program. */
        fun fromJson(json: String): ProgramBuilderDraft? = runCatching {
            val o = JSONObject(json)
            if (o.optInt("schema", 0) != SCHEMA) return null
            val dayArr = o.getJSONArray("days")
            val days = (0 until dayArr.length()).map { i ->
                val d = dayArr.getJSONObject(i)
                val exArr = d.getJSONArray("exercises")
                BuilderDay(
                    uid = d.getString("uid"),
                    key = d.getString("key"),
                    name = d.getString("name"),
                    archetype = d.getString("archetype"),
                    accentHex = d.getString("accent"),
                    word = d.optString("word", ""),
                    exercises = (0 until exArr.length()).map { j ->
                        val e = exArr.getJSONObject(j)
                        BuilderExercise(
                            uid = e.getString("uid"),
                            libId = e.getString("libId"),
                            name = e.getString("name"),
                            muscle = e.getString("muscle"),
                            sets = e.getInt("sets"),
                            reps = e.getString("reps")
                        )
                    }
                )
            }
            ProgramBuilderDraft(
                days = days,
                dirty = o.optBoolean("dirty", false),
                openDayUid = o.optString("openDay").ifBlank { null },
                dialog = dialogOf(o.optString("dialog", DIALOG_NONE), o.optString("dialogEx").ifBlank { null })
            )
        }.getOrNull()
    }
}
