package com.tally.core

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

    private fun reason(file: BackupFile): String? = (BackupCodec.validate(file) as? BackupReadResult.Invalid)?.reason

    private val bill = sample.recurring.first()

    private fun withBill(change: (RecurringDto) -> RecurringDto) =
        sample.copy(recurring = listOf(change(bill)) + sample.recurring.drop(1))

    @Test fun `the sample passes every check`() {
        assertEquals(null, reason(sample))
    }

    @Test fun `a bill that would throw where it is read is refused`() {
        assertEquals("A bill repeats at an interval outside 1 to 52.", reason(withBill { it.copy(interval = 0) }))
        assertEquals("A bill repeats at an interval outside 1 to 52.", reason(withBill { it.copy(interval = 60) }))
        assertEquals("A bill has an amount of zero or less.", reason(withBill { it.copy(amount = -5_00) }))
        assertEquals("A bill has an unreadable date.", reason(withBill { it.copy(nextDate = "soon") }))
        assertEquals("A bill has an unreadable date.", reason(withBill { it.copy(endDate = "2026-13-01") }))
    }

    @Test fun `a bill must point at what is in the file`() {
        assertEquals("A bill points at an account that is not in the file.", reason(withBill { it.copy(accountId = 404) }))
        assertEquals("A bill points at a category that is not in the file.", reason(withBill { it.copy(type = TxType.EXPENSE, categoryId = 404) }))
        val account = sample.accounts.first().id
        assertEquals(
            "A transfer bill needs two different accounts.",
            reason(withBill { it.copy(type = TxType.TRANSFER, accountId = account, toAccountId = null, categoryId = null) }),
        )
        assertEquals(
            "A transfer bill needs two different accounts.",
            reason(withBill { it.copy(type = TxType.TRANSFER, accountId = account, toAccountId = account, categoryId = null) }),
        )
    }

    @Test fun `an entry must point at a bill that is in the file`() {
        val orphan = sample.copy(transactions = sample.transactions + sample.transactions.first().copy(id = 99_999, recurringId = 404))
        assertEquals("A transaction points at a bill that is not in the file.", reason(orphan))
    }

    @Test fun `budgets and goals cannot be negative`() {
        assertEquals("A budget has a negative amount.", reason(sample.copy(budgets = sample.budgets + BudgetDto(99, null, -1))))
        assertEquals("A goal has a negative target.", reason(sample.copy(goals = sample.goals.map { it.copy(target = -1) })))
    }

    @Test fun `the month start and week start ride along, and an older file without them still reads`() {
        val payday = sample.copy(monthStartDay = 15, weekStartsMonday = false)
        val back = BackupCodec.decode(BackupCodec.encode(payday)) as BackupReadResult.Ok
        assertEquals(15, back.file.monthStartDay)
        assertEquals(false, back.file.weekStartsMonday)

        val older = BackupCodec.encode(sample).replace(Regex("""\s*"monthStartDay": null,"""), "").replace(Regex("""\s*"weekStartsMonday": null,"""), "")
        assertTrue(older, !older.contains("monthStartDay"))
        val read = BackupCodec.decode(older) as BackupReadResult.Ok
        assertEquals("a file that does not say leaves the setting alone", null, read.file.monthStartDay)
        assertEquals(null, read.file.weekStartsMonday)
    }

    @Test fun `a version 1 file, without goal kinds or values, still reads`() {
        val v1 = """{"format":"tally-backup","version":1,"exportedAt":"2026-10-04","currency":"CAD",
            "accounts":[{"id":1,"name":"Chequing","type":"CHEQUING","openingBalance":0}],
            "goals":[{"id":1,"name":"Trip","target":100000}]}"""
        val read = BackupCodec.decode(v1) as BackupReadResult.Ok
        assertEquals(GoalKind.SAVINGS, read.file.goals.single().kind)
        assertEquals(emptyList<AccountValueDto>(), read.file.values)
    }

    @Test fun `goal kinds and account values must point at what is in the file`() {
        assertEquals(
            "A goal points at an account that is not in the file.",
            reason(sample.copy(goals = sample.goals.map { it.copy(kind = GoalKind.BALANCE, accountId = 404) })),
        )
        assertEquals(
            "A goal asks for a share outside 0 to 100 percent.",
            reason(sample.copy(goals = sample.goals.map { it.copy(kind = GoalKind.INVEST, percent = 150) })),
        )
        assertEquals(
            "An account value points at an account that is not in the file.",
            reason(sample.copy(values = listOf(AccountValueDto(1, 404, "2026-10-01", 5_00)))),
        )
        assertEquals(
            "An account value has an unreadable date.",
            reason(sample.copy(values = listOf(AccountValueDto(1, sample.accounts.first().id, "later", 5_00)))),
        )
    }

    @Test fun `a version 2 file without investments still reads`() {
        val v2 = """{"format":"tally-backup","version":2,"exportedAt":"2026-10-04","currency":"CAD",
            "accounts":[{"id":1,"name":"TFSA","type":"INVESTMENT","openingBalance":0,"archived":false,"sortOrder":0}],
            "values":[{"id":1,"accountId":1,"date":"2026-10-01","value":500000}]}"""
        val read = BackupCodec.decode(v2) as BackupReadResult.Ok
        assertEquals(null, read.file.accounts.single().registration)
        assertEquals("", read.file.accounts.single().externalRef)
        assertEquals("A file from before uids were kept has none; the restore makes them", null, read.file.accounts.single().uid)
        assertEquals(emptyList<SecurityDto>(), read.file.securities)
        assertEquals(emptyList<ActivityDto>(), read.file.activities)
        assertEquals(emptyList<RoomFactDto>(), read.file.roomFacts)
        assertEquals(1, read.file.values.size)
    }

    private val invested = sample.copy(
        accounts = sample.accounts.map {
            if (it.type == AccountType.INVESTMENT) {
                it.copy(registration = Registration.TFSA, institution = "Wealthsimple", externalRef = "HQ7XFMC41CAD", uid = "ws:HQ7XFMC41CAD")
            } else {
                it
            }
        },
        securities = listOf(SecurityDto(1, "XEQT", "iShares Core Equity ETF Portfolio", "CAD", SecurityKind.ETF, "TSX", uid = "sec:XEQT")),
        holdings = listOf(HoldingDto(1, 5, 1, "2026-10-01", 1_000_000_000, 38_120, 38_120, uid = "hold:ws:HQ7XFMC41CAD:sec:XEQT:2026-10-01")),
        activities = listOf(
            ActivityDto(1, 5, 1, ActivityType.BUY, "2026-10-02", 100_000_000, 3_900, 0, "CAD", source = "WEALTHSIMPLE", uid = "imp:86fe65d31110bce166befa06e9747e9a"),
            ActivityDto(2, 5, null, ActivityType.FX, "2026-10-03", amount = 10_000, currency = "CAD", toAmount = 7_300, toCurrency = "USD"),
        ),
        prices = listOf(PriceDto(1, 1, "2026-10-01", 3_812_000_000, "IMPORT", uid = "px:sec:XEQT:2026-10-01")),
        fxRates = listOf(FxRateDto(1, "USD", "CAD", "2026-10-01", 137_125_000, "BANK_OF_CANADA", uid = "fx:USD:CAD:2026-10-01")),
        roomFacts = listOf(RoomFactDto(1, Registration.TFSA, 2026, 700_000, uid = "room:TFSA:2026")),
    )

    @Test fun `investments round trip`() {
        assertEquals(5L, sample.accounts.single { it.type == AccountType.INVESTMENT }.id)
        val text = BackupCodec.encode(invested)
        val back = BackupCodec.decode(text) as BackupReadResult.Ok
        assertEquals("Uids ride along, a missing one as null", invested, back.file)
        assertTrue(text, text.contains("\"externalRef\": \"HQ7XFMC41CAD\",\n            \"uid\": \"ws:HQ7XFMC41CAD\"\n"))
    }

    @Test fun `investments must point at what is in the file`() {
        assertEquals(null, reason(invested))
        assertEquals(
            "A holding points at an account that is not in the file.",
            reason(invested.copy(holdings = invested.holdings.map { it.copy(accountId = 404) })),
        )
        assertEquals(
            "A holding points at a security that is not in the file.",
            reason(invested.copy(holdings = invested.holdings.map { it.copy(securityId = 404) })),
        )
        assertEquals("A holding has an unreadable date.", reason(invested.copy(holdings = invested.holdings.map { it.copy(date = "May") })))
        assertEquals(
            "An activity has a negative amount.",
            reason(invested.copy(activities = invested.activities.map { it.copy(fee = -1) })),
        )
        assertEquals(
            "An activity points at an account that is not in the file.",
            reason(invested.copy(activities = invested.activities.map { it.copy(accountId = 404) })),
        )
        assertEquals(
            "An activity points at a security that is not in the file.",
            reason(invested.copy(activities = invested.activities.map { it.copy(securityId = 404) })),
        )
        assertEquals("An activity has an unreadable date.", reason(invested.copy(activities = invested.activities.map { it.copy(date = "") })))
        assertEquals(
            "A price points at a security that is not in the file.",
            reason(invested.copy(prices = invested.prices.map { it.copy(securityId = 404) })),
        )
        assertEquals("A price has an unreadable date.", reason(invested.copy(prices = invested.prices.map { it.copy(date = "2026-02-30") })))
        assertEquals("An exchange rate is zero or less.", reason(invested.copy(fxRates = invested.fxRates.map { it.copy(rate = 0) })))
        assertEquals(
            "An exchange rate has an unreadable date.",
            reason(invested.copy(fxRates = invested.fxRates.map { it.copy(date = "today") })),
        )
    }

    @Test fun `ignores unknown keys from a future minor change`() {
        val text = BackupCodec.encode(sample).replaceFirst("{", "{\"futureField\": true,")
        assertTrue(BackupCodec.decode(text) is BackupReadResult.Ok)
    }
}
