package com.forge.app.ui.nav

object Routes {
    const val OVERVIEW = "overview"
    const val GYM_TRAIN = "gym/train"
    const val GYM_STATS = "gym/stats"
    const val GYM_DAY = "gym/day/{dayKey}?skipWarmup={skipWarmup}"
    const val CARDIO = "cardio"
    const val TROPHIES = "trophies"
    const val NUTRITION = "nutrition"
    const val SETTINGS = "settings?page={page}"
    const val SESSION_HISTORY = "gym/session-history"
    const val SESSION_DETAIL = "gym/session-detail/{sessionId}"
    const val CARDIO_SESSION = "cardio/session/{cardioId}"
    const val NOTES_SEARCH = "gym/notes-search"
    const val RECAP = "recap"
    const val COACH_BRIEF = "coach-brief"
    const val COACH_LAB = "coach-lab"
    const val COACH_TIMELINE = "coach-timeline"
    const val PROFILE = "profile"
    const val GOALS = "goals"
    const val MIRROR_TEST = "mirror-test"
    const val PROGRAM_EDITOR = "program-editor/{dayKey}"
    const val PROGRAM_VIEWER = "program-viewer"
    const val PROGRAM_BUILDER = "program-builder?blank={blank}"
    const val FREESTYLE_LOG = "freestyle-log"

    const val ARG_BLANK = "blank"
    fun programBuilder(blank: Boolean = false) = "program-builder?blank=$blank"

    fun programEditor(dayKey: String) = "program-editor/$dayKey"

    const val ARG_DAY_KEY = "dayKey"
    const val ARG_SKIP_WARMUP = "skipWarmup"
    const val ARG_SESSION_ID = "sessionId"
    const val ARG_CARDIO_ID = "cardioId"
    const val ARG_SETTINGS_PAGE = "page"

    /** Settings, optionally deep-linked to a sub-page (pass a [SettingsPage] name; empty = root list). */
    fun settings(page: String? = null) = "settings?page=${page.orEmpty()}"

    fun gymDay(dayKey: String, skipWarmup: Boolean = false) =
        "gym/day/$dayKey?skipWarmup=$skipWarmup"

    fun sessionDetail(sessionId: Long) = "gym/session-detail/$sessionId"

    fun cardioSession(cardioId: Long) = "cardio/session/$cardioId"
}
