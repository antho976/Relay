package com.forge.app.ui.nav

object Routes {
    const val WELCOME = "welcome"
    const val OVERVIEW = "overview"
    const val GYM_TRAIN = "gym/train"
    const val GYM_STATS = "gym/stats"
    const val GYM_DAY = "gym/day/{dayKey}?skipWarmup={skipWarmup}"
    const val CARDIO = "cardio"
    const val TROPHIES = "trophies"
    const val NUTRITION = "nutrition"
    const val SETTINGS = "settings"
    const val SESSION_HISTORY = "gym/session-history"
    const val NOTES_SEARCH = "gym/notes-search"
    const val RECAP = "recap"
    const val ONBOARDING = "onboarding"
    const val PROGRAM_EDITOR = "program-editor/{dayKey}"

    fun programEditor(dayKey: String) = "program-editor/$dayKey"

    const val ARG_DAY_KEY = "dayKey"
    const val ARG_SKIP_WARMUP = "skipWarmup"

    fun gymDay(dayKey: String, skipWarmup: Boolean = false) =
        "gym/day/$dayKey?skipWarmup=$skipWarmup"
}
