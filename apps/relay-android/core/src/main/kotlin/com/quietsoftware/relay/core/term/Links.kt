package com.quietsoftware.relay.core.term

/** A web address as a program prints one; what ends a sentence after it is not part of it. */
private val URL = Regex("""https?://[^\s<>"'`│]+""")
private const val TRAILING = ".,;:!?)]}'\""

/**
 * The web address drawn at ([row], [col]), rows counted as [Terminal.lineAt] counts them,
 * followed across the rows the terminal wrapped it onto; null when there is none there.
 */
fun Terminal.urlAt(row: Int, col: Int): String? {
    val total = scrollbackSize + rows
    if (row !in 0 until total) return null
    var first = row
    while (first > 0 && lineAt(first - 1).wrapped) first--
    var last = row
    while (last < total - 1 && lineAt(last).wrapped) last++
    // The logical line's text, and for each of its chars the cell it was drawn in.
    val text = StringBuilder()
    val cells = ArrayList<Long>()
    for (r in first..last) {
        val line = lineAt(r)
        for (c in 0 until line.length) {
            val v = line.char(c)
            if (v == Line.WIDE_TAIL) continue
            val s = if (v <= 0) " " else line.text(c)
            repeat(s.length) { cells += cell(r, c) }
            text.append(s)
        }
    }
    val here = cell(row, col)
    for (m in URL.findAll(text)) {
        val url = m.value.trimEnd { it in TRAILING }
        if (url.length <= "https://".length) continue
        if (here in cells[m.range.first]..cells[m.range.first + url.length - 1]) return url
    }
    return null
}

/** A cell as one ordered number: row first, then column. */
private fun cell(row: Int, col: Int): Long = (row.toLong() shl 32) or col.toLong()

/** The word drawn at ([row], [col]): the run of non-blank cells around it, as start and end columns; null on a blank. */
fun Terminal.wordAt(row: Int, col: Int): IntRange? {
    if (row !in 0 until scrollbackSize + rows) return null
    val line = lineAt(row)
    fun blank(c: Int) = line.char(c).let { it <= 0 || it == ' '.code }
    if (col !in 0 until line.length || blank(col)) return null
    var from = col
    while (from > 0 && !blank(from - 1)) from--
    var to = col
    while (to < line.length - 1 && !blank(to + 1)) to++
    return from..to
}
