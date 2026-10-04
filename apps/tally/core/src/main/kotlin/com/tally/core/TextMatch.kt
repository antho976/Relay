package com.tally.core

/**
 * Case-insensitive matching that SQLite can run for every alphabet. SQLite's LIKE and
 * COLLATE NOCASE fold case for ASCII only, so a search for "épicerie" never finds "Épicerie".
 * GLOB is case-sensitive but takes character sets, so each letter of the query becomes the set
 * of its case forms ("[éÉ]") and the match still runs in SQL over every row, not over a capped
 * list. GLOB's own wildcards in the query are escaped, so "50%" or "a*b" match as typed.
 */
object TextMatch {

    /** A GLOB pattern for text that contains [query] in any case; empty when [query] is blank. */
    fun containsPattern(query: String): String {
        val q = query.trim()
        return if (q.isEmpty()) "" else "*" + caseless(q) + "*"
    }

    /** A GLOB pattern for text that starts with [prefix] in any case; empty when [prefix] is blank. */
    fun prefixPattern(prefix: String): String {
        val p = prefix.trim()
        return if (p.isEmpty()) "" else caseless(p) + "*"
    }

    private fun caseless(text: String): String {
        val sb = StringBuilder()
        text.forEach { c ->
            val forms = linkedSetOf(c, c.lowercaseChar(), c.uppercaseChar(), c.titlecaseChar())
            when {
                forms.size > 1 -> {
                    sb.append('[')
                    forms.forEach { sb.append(it) }
                    sb.append(']')
                }
                // The three characters GLOB reads as syntax outside a set; inside one they are plain.
                c == '*' || c == '?' || c == '[' -> sb.append('[').append(c).append(']')
                else -> sb.append(c)
            }
        }
        return sb.toString()
    }
}
