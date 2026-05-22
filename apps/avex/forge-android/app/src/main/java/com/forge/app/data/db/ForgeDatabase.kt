package com.forge.app.data.db

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.DayNameOverrideDao
import com.forge.app.data.db.dao.ExerciseCustomizationDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.UnlockedTrophyDao
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.DayNameOverride
import com.forge.app.data.db.entities.ExerciseCustomization
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.UnlockedTrophy

/**
 * Schema is currently v2 (v1 was the Phase 0 AppMeta placeholder, now removed).
 * Migrations are deliberately destructive until Antho logs his first "real" workout
 * — at that point we lock the schema, write real Migration objects per change, and
 * remove the destructive fallback in [com.forge.app.di.DatabaseModule].
 */
@Database(
    entities = [
        Session::class,
        LoggedExercise::class,
        LoggedSet::class,
        ExerciseCustomization::class,
        DayNameOverride::class,
        UnlockedTrophy::class,
        CardioEntry::class,
        MoodEntry::class
    ],
    version = 2,
    exportSchema = true
)
@TypeConverters(Converters::class)
abstract class ForgeDatabase : RoomDatabase() {
    abstract fun sessionDao(): SessionDao
    abstract fun loggedExerciseDao(): LoggedExerciseDao
    abstract fun loggedSetDao(): LoggedSetDao
    abstract fun exerciseCustomizationDao(): ExerciseCustomizationDao
    abstract fun dayNameOverrideDao(): DayNameOverrideDao
    abstract fun unlockedTrophyDao(): UnlockedTrophyDao
    abstract fun cardioDao(): CardioDao
    abstract fun moodDao(): MoodDao
}
