package com.quietsoftware.relay.core.term

/** A web address as a program prints one; what ends a sentence after it is not part of it. */
private val URL = Regex("""https?://[^\s<>"'`│]+""")
private const val TRAILING = ".,;:!?)]}'\""

/**
 * The web address drawn at ([row], [col]), rows counted as [Terminal.lineAt] counts them,
 * followed across the rows the terminal wrapped it onto, and across the ones a program broke it
 * onto itself (Claude Code wraps its own text: a long address fills a row to its last cell and
 * goes on after the next row's indent); null when there is none there.
 */
fun Terminal.urlAt(row: Int, col: Int): String? {
    val total = scrollbackSize + rows
    if (row !in 0 until total) return null
    fun joined(r: Int) = lineAt(r).wrapped || brokenAcross(lineAt(r), lineAt(r + 1))
    var first = row
    while (first > 0 && joined(first - 1)) first--
    var last = row
    while (last < total - 1 && joined(last)) last++
    // The logical line's text, and for each of its chars the cell it was drawn in.
    val text = StringBuilder()
    val cells = ArrayList<Long>()
    for (r in first..last) {
        val line = lineAt(r)
        // A row a program broke onto starts after its indent; one the terminal wrapped, at once.
        val from = if (r > first && !lineAt(r - 1).wrapped) (0 until line.length).firstOrNull { !blank(line.char(it)) } ?: 0 else 0
        for (c in from until line.length) {
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

/** The deepest indent a program's own continuation of an address may start after. */
private const val MAX_INDENT = 12

private fun blank(cp: Int) = cp <= 0 || cp == ' '.code

/** A character an address may hold. */
private fun urlChar(cp: Int) = cp in 0x21..0x7E && cp.toChar().let { it.isLetterOrDigit() || it in "-._~:/?#[]@!$&()*+,;=%" }

/** [prev] is full to its last cell with an address's characters and [next] goes on with more after an indent. */
private fun brokenAcross(prev: Line, next: Line): Boolean {
    if (prev.length == 0 || !urlChar(prev.char(prev.length - 1))) return false
    val start = (0 until next.length).firstOrNull { !blank(next.char(it)) } ?: return false
    return start <= MAX_INDENT && urlChar(next.char(start))
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
