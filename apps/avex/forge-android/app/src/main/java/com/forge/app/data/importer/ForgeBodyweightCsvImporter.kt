package com.forge.app.data.importer

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

    override fun parseExtras(text: String): ImportedExtras {
        val rows = CsvParser.parse(text)
        if (rows.size < 2) return ImportedExtras()
        val idx = ImportParsing.headerIndex(rows.first())
        val dateCol = ImportParsing.findCol(idx, "date") ?: return ImportedExtras()
        val weightCol = ImportParsing.findCol(idx, "weightlb", "weight_lb", "weight")
            ?: return ImportedExtras()

        val out = ArrayList<ImportedBodyweight>(rows.size - 1)
        for (row in rows.drop(1)) {
            val dateKey = ImportParsing.at(row, dateCol).trim()
            // The export writes the entry's own `date_key`, which is already yyyy-MM-dd and is the
            // column the unique index is on — so keep it verbatim rather than re-deriving a day
            // from a parsed instant in whatever zone this device happens to be in.
            if (!DATE_KEY.matches(dateKey)) continue
            val weightLb = ImportParsing.parseWeight(ImportParsing.at(row, weightCol))
            if (weightLb == null || weightLb <= 0.0) continue
            out.add(ImportedBodyweight(dateKey = dateKey, weightLb = weightLb))
        }
        return ImportedExtras(bodyweight = out)
    }

    private companion object {
        val DATE_KEY = Regex("""\d{4}-\d{2}-\d{2}""")
    }
}
