package com.margin.app.data.db

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
    ],
    version = 1,
    exportSchema = true,
)
@TypeConverters(Converters::class)
abstract class MarginDatabase : RoomDatabase() {
    abstract fun transactions(): TransactionDao
    abstract fun accounts(): AccountDao
    abstract fun categories(): CategoryDao
    abstract fun budgets(): BudgetDao
    abstract fun recurring(): RecurringDao
    abstract fun goals(): GoalDao

    companion object {
        const val NAME = "margin.db"
    }
}
