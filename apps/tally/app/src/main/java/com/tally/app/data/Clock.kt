package com.tally.app.data

import java.time.LocalDate
import java.time.ZoneId
import javax.inject.Inject
import javax.inject.Singleton

/** "Today" behind an interface, so tests and screenshots can pin the date. */
interface Clock {
    fun today(): LocalDate
    fun nowMillis(): Long
}

@Singleton
class SystemClock @Inject constructor() : Clock {
    override fun today(): LocalDate = LocalDate.now(ZoneId.systemDefault())
    override fun nowMillis(): Long = System.currentTimeMillis()
}
