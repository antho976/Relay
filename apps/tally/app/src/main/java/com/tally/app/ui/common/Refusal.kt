package com.tally.app.ui.common

/** "Not saved · Name the bill · Enter an amount": what stops the save, in form order; null when nothing does. */
fun refusalLine(problems: List<String?>): String? {
    val found = problems.filterNotNull()
    return if (found.isEmpty()) null else (listOf("Not saved") + found).joinToString(" · ")
}
