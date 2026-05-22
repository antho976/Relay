package com.forge.app.ui.nav

object Routes {
    const val WELCOME = "welcome"
    const val OVERVIEW = "overview"
    const val GYM_TRAIN = "gym/train"
    const val GYM_DAY = "gym/day/{dayKey}?skipWarmup={skipWarmup}"
    const val CARDIO = "cardio"
    const val TROPHIES = "trophies"

    const val ARG_DAY_KEY = "dayKey"
    const val ARG_SKIP_WARMUP = "skipWarmup"

    fun gymDay(dayKey: String, skipWarmup: Boolean = false) =
        "gym/day/$dayKey?skipWarmup=$skipWarmup"
}
