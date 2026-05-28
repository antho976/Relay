package com.forge.app.data.db

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import com.forge.app.data.db.dao.BodyweightDao
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.DayNameOverrideDao
import com.forge.app.data.db.dao.ExerciseCustomizationDao
import com.forge.app.data.db.dao.ExerciseGoalDao
import com.forge.app.data.db.dao.LoggedExerciseDao
import com.forge.app.data.db.dao.LoggedSetDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.RestDayDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.TrophyNearMissDao
import com.forge.app.data.db.dao.UnlockedTrophyDao
import com.forge.app.data.db.dao.ExtendedGoalDao
import com.forge.app.data.db.dao.ProgramCustomizationDao
import com.forge.app.data.db.dao.WarmupRoutineDao
import com.forge.app.data.db.dao.SessionBreakDao
import com.forge.app.data.db.dao.VacationDao
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.DayNameOverride
import com.forge.app.data.db.entities.ExerciseCustomization
import com.forge.app.data.db.entities.ExerciseGoal
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.MoodEntry
import com.forge.app.data.db.entities.RestDayEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.TrophyNearMiss
import com.forge.app.data.db.entities.UnlockedTrophy
import com.forge.app.data.db.entities.ExtendedGoal
import com.forge.app.data.db.entities.WarmupRoutineItem
import com.forge.app.data.db.entities.ProgramCustomization
import com.forge.app.data.db.entities.SessionBreak
import com.forge.app.data.db.entities.VacationPeriod

/**
 * Schema is currently v12 (v12 added LoggedSet.rpe; v11 added per-set annotations; prior versions are destructively migrated).
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
        MoodEntry::class,
        ExerciseGoal::class,
        RestDayEntry::class,
        TrophyNearMiss::class,
        BodyweightEntry::class,
        VacationPeriod::class,
        ExtendedGoal::class,
        SessionBreak::class,
        ProgramCustomization::class,
        WarmupRoutineItem::class
    ],
    version = 12,
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
    abstract fun exerciseGoalDao(): ExerciseGoalDao
    abstract fun restDayDao(): RestDayDao
    abstract fun trophyNearMissDao(): TrophyNearMissDao
    abstract fun bodyweightDao(): BodyweightDao
    abstract fun vacationDao(): VacationDao
    abstract fun extendedGoalDao(): ExtendedGoalDao
    abstract fun sessionBreakDao(): SessionBreakDao
    abstract fun programCustomizationDao(): ProgramCustomizationDao
    abstract fun warmupRoutineDao(): WarmupRoutineDao
}
