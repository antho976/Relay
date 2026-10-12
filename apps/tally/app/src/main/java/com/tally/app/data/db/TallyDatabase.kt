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
        TombstoneEntity::class,
        SyncStateEntity::class,
        SecurityEntity::class,
        HoldingEntity::class,
        ActivityEntity::class,
        PriceEntity::class,
        FxRateEntity::class,
        RoomFactEntity::class,
    ],
    version = 4,
    exportSchema = true,
    // 2: goal kinds (new goal columns with defaults) and investment account values (a new table).
    // 3: uids, change times and tombstones for sync, by hand (SyncSchema.MIGRATION_2_3): existing
    //    rows need a uid each, and the triggers that keep them are SQL Room does not write.
    // 4: investments (docs/INVESTMENTS.md): an account's registration, institution and number, and
    //    six synced tables, by hand (SyncSchema.MIGRATION_3_4) for the same triggers.
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
    abstract fun invest(): InvestDao
    abstract fun sync(): SyncDao

    companion object {
        const val NAME = "tally.db"
    }
}
