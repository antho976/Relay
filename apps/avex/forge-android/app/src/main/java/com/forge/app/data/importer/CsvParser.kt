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
     * Parse ONLY the header row — everything format detection actually needs.
     *
     * [parse]ing a whole file to look at its first line, discarding the result and parsing it again
     * is what made the Import screen's folder scan hostile: a 20 MB CSV that isn't a workout export
     * at all became ~1.5 M `String` cells (~150 MB of heap) TWICE before being rejected, once per
     * unrecognised file, on every visit to the screen.
     *
     * Returns null when there is no header, or no data row beneath it.
     */
    fun parseHeader(text: String): List<String>? {
        val clean = text.removePrefix("\uFEFF").trimStart('\n', '\r')
        if (clean.isBlank()) return null
        // A header with nothing under it is not an importable file, and the callers' resolve()
        // used to establish that from `rows.size < 2`.
        if (clean.lineSequence().filter { it.isNotBlank() }.take(2).count() < 2) return null
        val record = firstRecord(clean)
        return parseWith(record, detectDelimiter(clean)).firstOrNull()
    }

    /** The text up to the first record boundary — a newline that isn't inside a quoted field. */
    private fun firstRecord(text: String): String {
        var inQuotes = false
        for (i in text.indices) {
            val c = text[i]
            when {
                c == '"' -> inQuotes = !inQuotes
                !inQuotes && (c == '\n' || c == '\r') -> return text.substring(0, i)
            }
        }
        return text
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
                // A quote only OPENS a quoted field at the start of a field (RFC 4180). Treating
                // any quote as an opener meant one unescaped quote mid-cell — a note like
                // `paused 2" off chest` — flipped the parser into quote mode and swallowed every
                // following delimiter and newline into that one cell. With no closing quote, the
                // whole remainder of the file became a single cell: every later row silently
                // dropped, while the import summary reported a successful small import.
                c == '"' && cell.isEmpty() -> inQuotes = true
                c == delimiter -> { row.add(cell.toString()); cell.clear() }
                c == '\r' -> {
                    // CR-LF: let the LF terminate the row. A LONE CR is a row terminator too
                    // (classic-Mac line endings, and some exporters still emit them); swallowing it
                    // unconditionally collapsed such a file into one row, which then imported empty.
                    if (i + 1 >= n || text[i + 1] != '\n') {
                        row.add(cell.toString()); cell.clear()
                        rows.add(row); row = mutableListOf()
                    }
                }
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
