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
import com.forge.app.data.db.dao.ProgramDao
import com.forge.app.data.db.dao.RestEventDao
import com.forge.app.data.db.dao.AdviceEventDao
import com.forge.app.data.db.dao.CoachDao
import com.forge.app.data.db.dao.SuggestionOutcomeDao
import com.forge.app.data.db.dao.SessionSegmentDao
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
import com.forge.app.data.db.entities.ProgramDay
import com.forge.app.data.db.entities.ProgramSlot
import com.forge.app.data.db.entities.RestEvent
import com.forge.app.data.db.entities.AdviceEvent
import com.forge.app.data.db.entities.CoachPass
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.SuggestionOutcome
import com.forge.app.data.db.entities.SessionSegment
import com.forge.app.data.db.entities.SessionBreak
import com.forge.app.data.db.entities.VacationPeriod

/**
 * Schema is v21 (v21 added the `source` origin tag to program_customization +
 * exercise_customization for the auto-coach lock/undo fix; v20 added session_segment +
 * session.active_seconds for per-sitting timing;
 * v19 added the coach_decision propose/apply lifecycle columns; v18 added
 * suggestion_outcome for coach step calibration; v17 added the
 * auto-coach tables coach_pass/coach_decision; v16 added the adaptation-engine tables
 * rest_event/advice_event; v15 added a unique index on bodyweight_entry.date_key; v14 added
 * the data-driven program tables program_day/program_slot; v13 added an index on
 * LoggedExercise.exercise_id; v12 added LoggedSet.rpe; v11 added per-set annotations).
 *
 * The schema is now LOCKED from v12 onward: every change needs a real Migration in
 * [com.forge.app.data.db.Migrations] (registered in ALL_MIGRATIONS), a bumped version here,
 * an exported schema JSON, and a case in MigrationTest. Only the pre-lock versions (≤11) still
 * reset destructively (see [com.forge.app.di.DatabaseModule]). A version bump without a matching
 * migration fails loudly at startup — intended, so data is never silently wiped.
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
        WarmupRoutineItem::class,
        ProgramDay::class,
        ProgramSlot::class,
        RestEvent::class,
        AdviceEvent::class,
        CoachPass::class,
        CoachDecision::class,
        SuggestionOutcome::class,
        SessionSegment::class
    ],
    version = 21,
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
    abstract fun programDao(): ProgramDao
    abstract fun restEventDao(): RestEventDao
    abstract fun adviceEventDao(): AdviceEventDao
    abstract fun coachDao(): CoachDao
    abstract fun suggestionOutcomeDao(): SuggestionOutcomeDao
    abstract fun sessionSegmentDao(): SessionSegmentDao
}
