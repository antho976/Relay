package com.forge.app.data.importer

import java.time.LocalDate

/**
 * Reads back the bodyweight CSV Avex itself writes (`BackupRepository.exportBodyweightCsv`).
 *
 * That file has been offered as an export since weigh-ins existed and no importer ever read it, so
 * a user moving to a new phone by exporting everything they were offered left their entire weight
 * history behind — silently, since the file simply didn't match any parser and was reported as "not
 * a recognised gym-app export".
 *
 * It carries no workouts, so [parse] is empty by design and the weigh-ins arrive through
 * [parseExtras]. Ordered BEFORE [GenericCsvImporter] in the detector chain, though the generic
 * parser would decline it anyway for want of an exercise column.
 */
class ForgeBodyweightCsvImporter : GymImporter {
    override val source = ImportSource.FORGE_BODYWEIGHT_CSV

    override fun canParse(text: String): Boolean {
        val header = ImportParsing.firstLine(text)
        // Deliberately narrow: this is our own two-column file, not "any CSV with a weight in it".
        return header.contains("date") &&
            (header.contains("weightlb") || header.contains("weight_lb")) &&
            !header.contains("exercise")
    }

    /** No workouts in this file — the weigh-ins come back through [parseExtras]. */
    override fun parse(text: String, assumeKg: Boolean): List<ImportedSession> = emptyList()

    override fun parseExtras(text: String, assumeKg: Boolean): ImportedExtras = read(text, assumeKg).extras

    /**
     * Weigh-ins plus the rows that could not be one. Counted rather than dropped silently: a file
     * of nothing but bad rows used to report "No new workouts found", which reads as empty.
     */
    override fun read(text: String, assumeKg: Boolean): ParsedImport {
        val rows = CsvParser.parse(text)
        if (rows.size < 2) return ParsedImport(emptyList())
        val idx = ImportParsing.headerIndex(rows.first())
        val dateCol = ImportParsing.findCol(idx, "date") ?: return ParsedImport(emptyList())
        val weightCol = ImportParsing.findCol(idx, "weightlb", "weight_lb", "weight")
            ?: return ParsedImport(emptyList())

        val out = ArrayList<ImportedBodyweight>(rows.size - 1)
        var skipped = 0
        for (row in rows.drop(1)) {
            val dateKey = ImportParsing.at(row, dateCol).trim()
            // The export writes the entry's own `date_key`, which is already yyyy-MM-dd and is the
            // column the unique index is on — so keep it verbatim rather than re-deriving a day
            // from a parsed instant in whatever zone this device happens to be in.
            //
            // The shape alone admitted "2024-13-45", which then failed to parse wherever the key was
            // read back as a date: the weigh-in chart, and the insert's own recordedAt, which fell
            // back to the import instant for a day that does not exist.
            if (!DATE_KEY.matches(dateKey) || runCatching { LocalDate.parse(dateKey) }.isFailure) {
                skipped++
                continue
            }
            // NaN failed `<= 0.0` and was kept; Infinity and a 10x typo passed too, and one of them
            // flattens the weight chart's axis for good.
            val weightLb = ImportBounds.bodyweightLb(ImportParsing.parseWeight(ImportParsing.at(row, weightCol)))
            if (weightLb == null) {
                skipped++
                continue
            }
            out.add(ImportedBodyweight(dateKey = dateKey, weightLb = weightLb))
        }
        return ParsedImport(emptyList(), ImportedExtras(bodyweight = out), skipped)
    }

    private companion object {
        val DATE_KEY = Regex("""\d{4}-\d{2}-\d{2}""")
    }
}
