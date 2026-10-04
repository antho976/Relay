package com.margin.app.di

import android.content.Context
import androidx.room.Room
import com.margin.app.data.Clock
import com.margin.app.data.SystemClock
import com.margin.app.data.db.AccountDao
import com.margin.app.data.db.BudgetDao
import com.margin.app.data.db.CategoryDao
import com.margin.app.data.db.GoalDao
import com.margin.app.data.db.MarginDatabase
import com.margin.app.data.db.RecurringDao
import com.margin.app.data.db.TransactionDao
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
    fun database(@ApplicationContext context: Context): MarginDatabase =
        Room.databaseBuilder(context, MarginDatabase::class.java, MarginDatabase.NAME)
            // Version 1 has no migrations yet. When v2 lands, add a Migration and a test for it;
            // never fallbackToDestructiveMigration on a ledger.
            .build()

    @Provides fun transactions(db: MarginDatabase): TransactionDao = db.transactions()
    @Provides fun accounts(db: MarginDatabase): AccountDao = db.accounts()
    @Provides fun categories(db: MarginDatabase): CategoryDao = db.categories()
    @Provides fun budgets(db: MarginDatabase): BudgetDao = db.budgets()
    @Provides fun recurring(db: MarginDatabase): RecurringDao = db.recurring()
    @Provides fun goals(db: MarginDatabase): GoalDao = db.goals()

    @Provides @Singleton @AppScope
    fun appScope(): CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
}

@Module
@InstallIn(SingletonComponent::class)
abstract class ClockModule {
    @Binds abstract fun clock(impl: SystemClock): Clock
}
