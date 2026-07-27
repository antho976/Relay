package com.forge.app.data.db

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import com.forge.app.data.db.dao.BodyFatDao
import com.forge.app.data.db.dao.LeanMassDao
import com.forge.app.data.db.dao.SessionHrSampleDao
import com.forge.app.data.db.dao.BodyMeasurementDao
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
import com.forge.app.data.db.dao.CheckinDao
import com.forge.app.data.db.dao.CoachProjectDao
import com.forge.app.data.db.dao.TrainingBlockDao
import com.forge.app.data.db.dao.CoachGoalDao
import com.forge.app.data.db.dao.InjuryRestrictionDao
import com.forge.app.data.db.dao.LessonEventDao
import com.forge.app.data.db.dao.SuggestionOutcomeDao
import com.forge.app.data.db.dao.SessionSegmentDao
import com.forge.app.data.db.dao.WarmupRoutineDao
import com.forge.app.data.db.dao.SessionBreakDao
import com.forge.app.data.db.dao.VacationDao
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.db.entities.LeanMassEntry
import com.forge.app.data.db.entities.BodyMeasurementEntry
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.DayNameOverride
import com.forge.app.data.db.entities.ExerciseCustomization
import com.forge.app.data.db.entities.ExerciseGoal
import com.forge.app.data.db.entities.InjuryRestriction
import com.forge.app.data.db.entities.LessonEvent
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
import com.forge.app.data.db.entities.CheckinEntry
import com.forge.app.data.db.entities.CoachGoal
import com.forge.app.data.db.entities.CoachPass
import com.forge.app.data.db.entities.CoachProject
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.SuggestionOutcome
import com.forge.app.data.db.entities.SessionHrSample
import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.data.db.entities.SessionSegment
import com.forge.app.data.db.entities.SessionBreak
import com.forge.app.data.db.entities.VacationPeriod

/**
 * Schema is v31 (v31 added `session_hr_sample` — the watch's live HR stream per session for W3,
 * a new empty table, additive, CASCADE with its session;
 * v30 added `lean_mass` — per-day lean-body-mass history for W6, a new empty
 * table, additive; import-only from Health Connect (a watch's BIA measurement), sibling of `body_fat`;
 * v29 added `body_fat` — per-day body-fat-% history for GYMAP-62, a new empty
 * table, additive; readings come from Health Connect (a smart scale) or manual entry;
 * v28 added `cardio_entry.conditions` — comma-joined weather/environment tags
 * (hot/cold/rain/wind) a session was done in for GYMAP-39, an additive nullable column; null = none tagged;
 * v27 added `cardio_entry.incline_pct` + `cardio_entry.laps` + `cardio_entry.elevation_m`
 * — per-type optional cardio fields (treadmill grade / pool laps / outdoor elevation gain) for GYMAP-38,
 * three additive nullable columns; null = the field doesn't apply / wasn't logged;
 * v26 added `logged_set.duration_seconds` — held time in seconds for timed-hold sets
 * (planks/dead hangs/wall sits) for GYMAP-51, an additive nullable column; null = normal rep-based set;
 * v25 added `bodyweight_entry.note` — an optional per-weigh-in note for GYMAP-54, an
 * additive nullable column; v24 added `body_measurement` — per-type circumference history for GYMAP-52, a new
 * empty table, additive; v23 added `cardio_entry.interval_count` + `cardio_entry.hr_zone` for cardio
 * depth — HIIT interval counts + manual HR-zone tags;
 * v22 added `logged_exercise.slot_id` + `exercise_customization.swapped_exercise_id`
 * for swap re-attribution, so a swapped entry's `exercise_id` becomes the real exercise performed and
 * its PRs/stats attribute correctly; v21 added the `source` origin tag to program_customization +
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
        BodyFatEntry::class,
        LeanMassEntry::class,
        BodyMeasurementEntry::class,
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
        SessionSegment::class,
        SessionHrSample::class,
        CoachGoal::class,
        LessonEvent::class,
        CheckinEntry::class,
        InjuryRestriction::class,
        TrainingBlock::class,
        CoachProject::class
    ],
    version = 35,
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
    abstract fun bodyFatDao(): BodyFatDao
    abstract fun leanMassDao(): LeanMassDao
    abstract fun sessionHrSampleDao(): SessionHrSampleDao
    abstract fun bodyMeasurementDao(): BodyMeasurementDao
    abstract fun vacationDao(): VacationDao
    abstract fun extendedGoalDao(): ExtendedGoalDao
    abstract fun sessionBreakDao(): SessionBreakDao
    abstract fun programCustomizationDao(): ProgramCustomizationDao
    abstract fun warmupRoutineDao(): WarmupRoutineDao
    abstract fun programDao(): ProgramDao
    abstract fun restEventDao(): RestEventDao
    abstract fun adviceEventDao(): AdviceEventDao
    abstract fun coachDao(): CoachDao
    abstract fun coachGoalDao(): CoachGoalDao
    abstract fun checkinDao(): CheckinDao
    abstract fun trainingBlockDao(): TrainingBlockDao
    abstract fun coachProjectDao(): CoachProjectDao
    abstract fun injuryRestrictionDao(): InjuryRestrictionDao
    abstract fun lessonEventDao(): LessonEventDao
    abstract fun suggestionOutcomeDao(): SuggestionOutcomeDao
    abstract fun sessionSegmentDao(): SessionSegmentDao
}
