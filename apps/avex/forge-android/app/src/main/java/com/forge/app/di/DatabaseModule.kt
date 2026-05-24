package com.forge.app.di

import android.content.Context
import androidx.room.Room
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.BodyweightDao
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.DayNameOverrideDao
import com.forge.app.data.db.dao.ExerciseCustomizationDao
import com.forge.app.data.db.dao.ExerciseGoalDao
import com.forge.app.data.db.dao.RestDayDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.TrophyNearMissDao
import com.forge.app.data.db.dao.UnlockedTrophyDao
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {

    @Provides
    @Singleton
    fun provideDatabase(@ApplicationContext context: Context): ForgeDatabase =
        Room.databaseBuilder(context, ForgeDatabase::class.java, "forge.db")
            // Destructive until first real workout is logged. See ForgeDatabase docs.
            .fallbackToDestructiveMigration(dropAllTables = true)
            .build()

    @Provides fun provideSessionDao(db: ForgeDatabase): SessionDao = db.sessionDao()
    @Provides fun provideLoggedExerciseDao(db: ForgeDatabase): LoggedExerciseDao = db.loggedExerciseDao()
    @Provides fun provideLoggedSetDao(db: ForgeDatabase): LoggedSetDao = db.loggedSetDao()
    @Provides fun provideExerciseCustomizationDao(db: ForgeDatabase): ExerciseCustomizationDao = db.exerciseCustomizationDao()
    @Provides fun provideDayNameOverrideDao(db: ForgeDatabase): DayNameOverrideDao = db.dayNameOverrideDao()
    @Provides fun provideUnlockedTrophyDao(db: ForgeDatabase): UnlockedTrophyDao = db.unlockedTrophyDao()
    @Provides fun provideCardioDao(db: ForgeDatabase): CardioDao = db.cardioDao()
    @Provides fun provideMoodDao(db: ForgeDatabase): MoodDao = db.moodDao()
    @Provides fun provideExerciseGoalDao(db: ForgeDatabase): ExerciseGoalDao = db.exerciseGoalDao()
    @Provides fun provideRestDayDao(db: ForgeDatabase): RestDayDao = db.restDayDao()
    @Provides fun provideTrophyNearMissDao(db: ForgeDatabase): TrophyNearMissDao = db.trophyNearMissDao()
    @Provides fun provideBodyweightDao(db: ForgeDatabase): BodyweightDao = db.bodyweightDao()
    @Provides fun provideVacationDao(db: ForgeDatabase): com.forge.app.data.db.dao.VacationDao = db.vacationDao()
    @Provides fun provideExtendedGoalDao(db: ForgeDatabase): com.forge.app.data.db.dao.ExtendedGoalDao = db.extendedGoalDao()
    @Provides fun provideSessionBreakDao(db: ForgeDatabase): com.forge.app.data.db.dao.SessionBreakDao = db.sessionBreakDao()
    @Provides fun provideProgramCustomizationDao(db: ForgeDatabase): com.forge.app.data.db.dao.ProgramCustomizationDao = db.programCustomizationDao()
    @Provides fun provideWarmupRoutineDao(db: ForgeDatabase): com.forge.app.data.db.dao.WarmupRoutineDao = db.warmupRoutineDao()
}
