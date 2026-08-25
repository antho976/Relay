package com.forge.app.data.importer

/**
 * Minimal RFC 4180 CSV reader for gym-app exports (#GYMAP-17). Handles quoted fields, escaped
 * doubled quotes, and both `,` and `;` delimiters (Strong's European exports use `;`). Returns rows
 * of raw string cells; higher layers map columns by header name.
 *
 * Deliberately tolerant — a malformed line yields the cells it can rather than throwing, since a
 * single bad row in a multi-year export shouldn't sink the whole import.
 */
object CsvParser {

    /** Parse [text] into rows of cells, auto-detecting the delimiter from the header line. */
    fun parse(text: String): List<List<String>> {
        val clean = text.removePrefix("\uFEFF") // strip a UTF-8 BOM some apps prepend
        if (clean.isBlank()) return emptyList()
        val delimiter = detectDelimiter(clean)
        return parseWith(clean, delimiter)
    }

    /**
     * Pick the delimiter by counting `,` vs `;` OUTSIDE quotes on the first non-empty line. Comma is
     * the default; semicolon only wins when it clearly dominates (locale exports).
     */
    private fun detectDelimiter(text: String): Char {
        val firstLine = text.lineSequence().firstOrNull { it.isNotBlank() } ?: return ','
        var commas = 0
        var semis = 0
        var inQuotes = false
        for (c in firstLine) {
            when {
                c == '"' -> inQuotes = !inQuotes
                !inQuotes && c == ',' -> commas++
                !inQuotes && c == ';' -> semis++
            }
        }
        return if (semis > commas) ';' else ','
    }

    private fun parseWith(text: String, delimiter: Char): List<List<String>> {
        val rows = mutableListOf<List<String>>()
        val cell = StringBuilder()
        var row = mutableListOf<String>()
        var inQuotes = false
        var i = 0
        val n = text.length
        while (i < n) {
            val c = text[i]
            when {
                inQuotes -> when {
                    c == '"' && i + 1 < n && text[i + 1] == '"' -> { cell.append('"'); i++ }
                    c == '"' -> inQuotes = false
                    else -> cell.append(c)
                }
                c == '"' -> inQuotes = true
                c == delimiter -> { row.add(cell.toString()); cell.clear() }
                c == '\r' -> { /* swallow; the paired \n ends the row */ }
                c == '\n' -> {
                    row.add(cell.toString()); cell.clear()
                    rows.add(row); row = mutableListOf()
                }
                else -> cell.append(c)
            }
            i++
        }
        // Flush the trailing cell/row when the file doesn't end with a newline.
        if (cell.isNotEmpty() || row.isNotEmpty()) {
            row.add(cell.toString())
            rows.add(row)
        }
        // Drop fully-blank rows (trailing newlines, blank separators between sections).
        return rows.filterNot { cells -> cells.all { it.isBlank() } }
    }
}
