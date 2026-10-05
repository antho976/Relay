package com.tally.app.data.db

import androidx.room.AutoMigration
import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters

@Database(
    entities = [
        AccountEntity::class,
        CategoryEntity::class,
        TransactionEntity::class,
        BudgetEntity::class,
        RecurringEntity::class,
        GoalEntity::class,
        ContributionEntity::class,
        AccountValueEntity::class,
    ],
    version = 2,
    exportSchema = true,
    // 2: goal kinds (new goal columns with defaults) and investment account values (a new table).
    autoMigrations = [AutoMigration(from = 1, to = 2)],
)
@TypeConverters(Converters::class)
abstract class TallyDatabase : RoomDatabase() {
    abstract fun transactions(): TransactionDao
    abstract fun accounts(): AccountDao
    abstract fun categories(): CategoryDao
    abstract fun budgets(): BudgetDao
    abstract fun recurring(): RecurringDao
    abstract fun goals(): GoalDao
    abstract fun values(): AccountValueDao

    companion object {
        const val NAME = "tally.db"
    }
}
