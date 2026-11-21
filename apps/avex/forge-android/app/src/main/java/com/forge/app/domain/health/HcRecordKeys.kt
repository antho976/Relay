package com.forge.app.domain.health

/**
 * The `clientRecordId` scheme for every record Avex authors in Health Connect (M-02).
 *
 * Health Connect upserts and deletes Avex's own records by (our package, clientRecordId), so a
 * record is only ever addressable again if its key can be re-derived from local state. Each key
 * is built from the local row's identity: the Room id for sessions and cardio entries, the
 * one-per-day `date_key` for weight and body fat (their tables upsert by day with INSERT OR
 * REPLACE, which hands a re-saved day a fresh row id, so the day is the stable identity there).
 *
 * The session and cardio strings are a wire contract with records already written by earlier
 * builds. Changing one would orphan every mirror written under the old spelling.
 *
 * Pure and Android-free so the scheme is unit-testable and shared by the writers (the
 * repositories) and the deleters (the same repositories plus the reset path).
 */
object HcRecordKeys {

    /** A finished gym session's ExerciseSessionRecord. */
    fun session(sessionId: Long): String = "avex-session-$sessionId"

    /** A finished gym session's ActiveCaloriesBurnedRecord. */
    fun sessionCalories(sessionId: Long): String = "avex-session-kcal-$sessionId"

    /** A finished gym session's HeartRateRecord series (the watch trace). */
    fun sessionHeartRate(sessionId: Long): String = "avex-session-hr-$sessionId"

    /** A non-rest cardio entry's ExerciseSessionRecord. */
    fun cardio(entryId: Long): String = "avex-cardio-$entryId"

    /** A same-day weigh-in's WeightRecord, keyed on the entry's ISO `date_key`. */
    fun weight(dateKey: String): String = "avex-weight-$dateKey"

    /** A same-day body-fat entry's BodyFatRecord, keyed on the entry's ISO `date_key`. */
    fun bodyFat(dateKey: String): String = "avex-bodyfat-$dateKey"

    /**
     * Every key a gym session can have written, by record type, for the [sessionIds] given. The
     * reset path and a single history delete both fan out through this so the three mirrors can
     * never drift apart (a session removed locally must take its calories and HR trace with it).
     */
    data class SessionMirrorKeys(
        val sessions: List<String>,
        val calories: List<String>,
        val heartRate: List<String>
    )

    fun sessionMirrors(sessionIds: Collection<Long>): SessionMirrorKeys = SessionMirrorKeys(
        sessions = sessionIds.map(::session),
        calories = sessionIds.map(::sessionCalories),
        heartRate = sessionIds.map(::sessionHeartRate)
    )
}
