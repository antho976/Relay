package com.forge.app.ui.nav

object Routes {
    const val WELCOME = "welcome"
    const val OVERVIEW = "overview"
    const val GYM_TRAIN = "gym/train"
    const val GYM_DAY = "gym/day/{dayKey}"
    const val CARDIO = "cardio"
    const val TROPHIES = "trophies"

    fun gymDay(dayKey: String) = "gym/day/$dayKey"

    const val ARG_DAY_KEY = "dayKey"
}
