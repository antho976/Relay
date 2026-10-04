package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

class SampleDataTest {

    private val today = LocalDate.of(2026, 10, 4)
    private val file = SampleData.build(today, 2, "CAD")

    @Test fun `nothing is dated in the future`() {
        assertTrue(file.transactions.all { !LocalDate.parse(it.date).isAfter(today) })
    }

    @Test fun `every account says it is sample data`() {
        assertTrue(file.accounts.all { it.name.startsWith(SampleData.ACCOUNT_PREFIX) })
    }

    @Test fun `is deterministic`() {
        assertEquals(file, SampleData.build(today, 2, "CAD"))
    }

    @Test fun `covers three months with ids unique`() {
        val dates = file.transactions.map { LocalDate.parse(it.date) }
        assertTrue(dates.minOrNull()!! <= today.withDayOfMonth(1).minusMonths(2))
        assertEquals(file.transactions.size, file.transactions.map { it.id }.toSet().size)
    }

    @Test fun `passes backup validation`() {
        assertTrue(BackupCodec.validate(file) is BackupReadResult.Ok)
    }

    @Test fun `recurring next dates are after today`() {
        assertTrue(file.recurring.all { LocalDate.parse(it.nextDate).isAfter(today) })
    }
}
