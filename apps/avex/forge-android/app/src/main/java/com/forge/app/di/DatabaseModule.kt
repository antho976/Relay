package com.forge.app.di

import android.content.Context
import androidx.room.Room
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.DayNameOverrideDao
import com.forge.app.data.db.dao.ExerciseCustomizationDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.SessionDao
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
}
