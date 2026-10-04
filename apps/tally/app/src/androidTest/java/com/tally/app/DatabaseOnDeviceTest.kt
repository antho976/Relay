package com.tally.app

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.TransactionEntity
import com.tally.core.AccountType
import com.tally.core.CategoryKind
import com.tally.core.TxType
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import java.time.LocalDate

/**
 * The real device SQLite, not Robolectric's: foreign keys, cascades and the balance query on the
 * platform build of SQLite the app actually ships against.
 */
@RunWith(AndroidJUnit4::class)
class DatabaseOnDeviceTest {

    private val db = Room.inMemoryDatabaseBuilder(ApplicationProvider.getApplicationContext(), TallyDatabase::class.java).build()

    @After fun close() = db.close()

    @Test fun balancesAndCascadesOnTheDeviceSqlite() = runTest {
        val chequing = db.accounts().insert(AccountEntity(name = "Chequing", type = AccountType.CHEQUING, openingBalance = 100_00))
        val visa = db.accounts().insert(AccountEntity(name = "Visa", type = AccountType.CREDIT))
        val food = db.categories().insert(CategoryEntity(name = "Food", kind = CategoryKind.EXPENSE, color = 0, icon = "cart"))
        val day = LocalDate.of(2026, 10, 4)
        db.transactions().insert(TransactionEntity(type = TxType.EXPENSE, amount = 30_00, date = day, accountId = visa, categoryId = food))
        db.transactions().insert(TransactionEntity(type = TxType.TRANSFER, amount = 20_00, date = day, accountId = chequing, toAccountId = visa))
        db.transactions().insert(TransactionEntity(type = TxType.INCOME, amount = 50_00, date = day, accountId = chequing))

        val balances = db.accounts().observeBalances().first().associate { it.name to it.balance }
        assertEquals(130_00L, balances["Chequing"])
        assertEquals(-10_00L, balances["Visa"])

        db.categories().delete(food)
        assertEquals(null, db.transactions().all().first { it.type == TxType.EXPENSE }.categoryId)

        db.accounts().delete(db.accounts().get(visa)!!)
        assertEquals(1, db.transactions().count())
    }
}
