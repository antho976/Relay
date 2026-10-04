package com.margin.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class BackupCodecTest {

    private val sample = SampleData.build(LocalDate.of(2026, 10, 4), 2, "CAD")

    @Test fun `sample data round trips`() {
        val text = BackupCodec.encode(sample)
        val back = BackupCodec.decode(text)
        assertTrue(back.toString(), back is BackupReadResult.Ok)
        assertEquals(sample, (back as BackupReadResult.Ok).file)
    }

    @Test fun `rejects files that are not backups`() {
        assertTrue(BackupCodec.decode("{\"hello\":1}") is BackupReadResult.Invalid)
        assertTrue(BackupCodec.decode("not json") is BackupReadResult.Invalid)
    }

    @Test fun `rejects a newer version with a reason that says what to do`() {
        val text = BackupCodec.encode(sample.copy(version = BackupFile.VERSION + 1))
        val r = BackupCodec.decode(text) as BackupReadResult.Invalid
        assertTrue(r.reason, r.reason.contains("Update the app"))
    }

    @Test fun `rejects dangling references`() {
        val broken = sample.copy(transactions = sample.transactions + sample.transactions.first().copy(id = 99_999, accountId = 404))
        assertTrue(BackupCodec.validate(broken) is BackupReadResult.Invalid)
    }

    @Test fun `ignores unknown keys from a future minor change`() {
        val text = BackupCodec.encode(sample).replaceFirst("{", "{\"futureField\": true,")
        assertTrue(BackupCodec.decode(text) is BackupReadResult.Ok)
    }
}
