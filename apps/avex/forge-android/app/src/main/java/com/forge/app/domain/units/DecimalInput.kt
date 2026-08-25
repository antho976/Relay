package com.forge.app.domain.units

/**
 * Decimal handling for numbers the USER types.
 *
 * `String.toDoubleOrNull()` delegates to `java.lang.Double.parseDouble`, which is
 * locale-INDEPENDENT and accepts only `.` as the decimal separator. But `KeyboardType.Decimal`
 * shows whatever separator the device locale uses, so on a comma-decimal locale — de, fr, es, it,
 * pt, nl, and most of Europe and South America — the user types "82,5" and every parser in the app
 * returned null for it.
 *
 * Null is not a visible failure here: the raw text is stored and displayed verbatim, so the row
 * still reads "82,5" while the parsed weight is absent. The set contributes nothing to volume and
 * can never register a PR, silently and permanently.
 */

/**
 * Normalise typed input into the form `toDoubleOrNull` accepts.
 *
 * Rules, in order:
 *  - exactly one `,` and no `.` → the comma IS the decimal separator ("82,5" → "82.5")
 *  - both present               → `,` is grouping, `.` is the decimal ("1,250.5" → "1250.5")
 *  - only `.` or neither        → unchanged
 *
 * The first rule is the safe reading for the fields this serves: nobody types a bare grouping
 * separator into a weight, bodyweight or measurement field, so a lone comma is always a decimal
 * point. (An importer reading a machine-written CSV cannot assume that — see GymImporter, where a
 * lone comma may genuinely be a thousands separator.)
 */
fun normalizeDecimalInput(input: String): String {
    val t = input.trim()
    if (!t.contains(',')) return t
    return if (t.contains('.')) t.replace(",", "") else t.replace(',', '.')
}

/**
 * Filter for a decimal text field: digits plus at most ONE decimal separator.
 *
 * Accepts the comma key a comma-locale keyboard produces and normalises it to `.` as it is typed,
 * so the field's own content is always parseable and there is one canonical form to store. The
 * previous filters dropped `,` outright, which turned "82,5" into "825" — a plausible-looking
 * number that then failed range validation, or worse, did not.
 *
 * Extra separators collapse rather than being rejected, so "7.5.2" cannot slip through and surface
 * as a misleading out-of-range error.
 */
fun filterDecimalInput(raw: String): String {
    val f = buildString {
        for (ch in raw) {
            when {
                ch.isDigit() -> append(ch)
                ch == '.' || ch == ',' -> append('.')
            }
        }
    }
    val dot = f.indexOf('.')
    return if (dot < 0) f else f.substring(0, dot + 1) + f.substring(dot + 1).replace(".", "")
}
