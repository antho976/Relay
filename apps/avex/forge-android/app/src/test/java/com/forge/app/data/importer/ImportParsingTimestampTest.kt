package com.forge.app.data.importer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test
import java.time.Instant
import java.time.LocalDateTime
import java.time.ZoneId

/**
 * A timestamp that states its own zone must import as the instant it names.
 *
 * The old format list matched "yyyy-MM-dd'T'HH:mm:ss'Z'" with the Z quoted, so the UTC designator
 * was treated as a literal character: the wall time was taken at face value and then re-zoned into
 * the device's zone. Every imported session shifted by the local offset, which for anyone far
 * enough east or west moved sessions onto the wrong calendar day and the wrong ISO week.
 */
class ImportParsingTimestampTest {

    @Test
    fun utcDesignatorIsReadAsUtc() {
        val parsed = ImportParsing.parseEpochMillis("2026-08-25T10:00:00Z")
        assertEquals(Instant.parse("2026-08-25T10:00:00Z").toEpochMilli(), parsed)
    }

    @Test
    fun explicitOffsetIsHonoured() {
        // Previously matched no format at all, so the row was dropped as unparseable.
        val parsed = ImportParsing.parseEpochMillis("2026-08-25T12:00:00+02:00")
        assertEquals(Instant.parse("2026-08-25T10:00:00Z").toEpochMilli(), parsed)
    }

    @Test
    fun offsetAndUtcFormsAgreeOnTheSameInstant() {
        assertEquals(
            ImportParsing.parseEpochMillis("2026-08-25T10:00:00Z"),
            ImportParsing.parseEpochMillis("2026-08-25T12:00:00+02:00")
        )
    }

    @Test
    fun aZonelessTimestampIsStillLocal() {
        // No offset stated, so local wall time remains the correct reading.
        val expected = LocalDateTime.of(2026, 8, 25, 10, 0, 0)
            .atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()
        assertEquals(expected, ImportParsing.parseEpochMillis("2026-08-25T10:00:00"))
        assertEquals(expected, ImportParsing.parseEpochMillis("2026-08-25 10:00:00"))
    }

    @Test
    fun dateOnlyStillParses() {
        assertNotNull(ImportParsing.parseEpochMillis("2026-08-25"))
    }

    @Test
    fun garbageStillReturnsNull() {
        assertEquals(null, ImportParsing.parseEpochMillis(""))
        assertEquals(null, ImportParsing.parseEpochMillis("not a date"))
    }
}
