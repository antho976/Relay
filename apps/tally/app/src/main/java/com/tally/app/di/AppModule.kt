package com.tally.app.di

import android.content.Context
import androidx.room.Room
import com.tally.app.data.Clock
import com.tally.app.data.SystemClock
import com.tally.app.data.db.AccountDao
import com.tally.app.data.db.AccountValueDao
import com.tally.app.data.db.BudgetDao
import com.tally.app.data.db.CategoryDao
import com.tally.app.data.db.GoalDao
import com.tally.app.data.db.InvestDao
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.RecurringDao
import com.tally.app.data.db.TransactionDao
import com.tally.app.data.db.tally
import dagger.Binds
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import javax.inject.Qualifier
import javax.inject.Singleton

/** App-lifetime scope for work that must outlive a screen (an undo restore, a backup write). */
@Qualifier @Retention(AnnotationRetention.BINARY) annotation class AppScope

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {

    @Provides @Singleton
    fun database(@ApplicationContext context: Context): TallyDatabase =
        Room.databaseBuilder(context, TallyDatabase::class.java, TallyDatabase.NAME)
            // Version 2 arrives by Room's auto-migration, versions 3 and 4 by SyncSchema's (all
            // tested in MigrationTest), and tally() adds the sync triggers to a fresh install. Never
            // fallbackToDestructiveMigration on a ledger.
            .tally()
            .build()

    @Provides fun transactions(db: TallyDatabase): TransactionDao = db.transactions()
    @Provides fun accounts(db: TallyDatabase): AccountDao = db.accounts()
    @Provides fun categories(db: TallyDatabase): CategoryDao = db.categories()
    @Provides fun budgets(db: TallyDatabase): BudgetDao = db.budgets()
    @Provides fun recurring(db: TallyDatabase): RecurringDao = db.recurring()
    @Provides fun goals(db: TallyDatabase): GoalDao = db.goals()
    @Provides fun values(db: TallyDatabase): AccountValueDao = db.values()
    @Provides fun invest(db: TallyDatabase): InvestDao = db.invest()

    @Provides @Singleton @AppScope
    fun appScope(): CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
}

@Module
@InstallIn(SingletonComponent::class)
abstract class ClockModule {
    @Binds abstract fun clock(impl: SystemClock): Clock
}
