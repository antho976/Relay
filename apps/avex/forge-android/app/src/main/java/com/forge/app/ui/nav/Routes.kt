package com.forge.app.ui.nav

object Routes {
    const val OVERVIEW = "overview"
    const val GYM_TRAIN = "gym/train"
    const val GYM_STATS = "gym/stats"
    const val GYM_DAY = "gym/day/{dayKey}?skipWarmup={skipWarmup}"
    const val CARDIO = "cardio"
    const val TROPHIES = "trophies"
    const val NUTRITION = "nutrition"
    const val SETTINGS = "settings"
    const val SESSION_HISTORY = "gym/session-history"
    const val SESSION_DETAIL = "gym/session-detail/{sessionId}"
    const val NOTES_SEARCH = "gym/notes-search"
    const val RECAP = "recap"
    const val COACH_BRIEF = "coach-brief"
    const val PROFILE = "profile"
    const val PROGRAM_EDITOR = "program-editor/{dayKey}"
    const val PROGRAM_VIEWER = "program-viewer"

    fun programEditor(dayKey: String) = "program-editor/$dayKey"

    const val ARG_DAY_KEY = "dayKey"
    const val ARG_SKIP_WARMUP = "skipWarmup"
    const val ARG_SESSION_ID = "sessionId"

    fun gymDay(dayKey: String, skipWarmup: Boolean = false) =
        "gym/day/$dayKey?skipWarmup=$skipWarmup"

    fun sessionDetail(sessionId: Long) = "gym/session-detail/$sessionId"
}
