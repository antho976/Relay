package com.forge.app.domain.coach

/**
 * The goal catalogue (Coach v3 A2) — every objective the coach knows how to measure, chase and
 * argue about. Deliberately bounded: a kind exists only when the app already logs a number that
 * moves it, because a goal the coach can't read is a wish.
 *
 * The vocabulary intentionally overlaps the older `extended_goal` types (`1rm`, `weekly_volume`,
 * `frequency`) and `GoalMetric` (`cardio_minutes`, `bodyweight`) — those rows stay where they are
 * and can be promoted into a coach goal one tap at a time.
 */
enum class CoachGoalKind(
    val code: String,
    val displayName: String,
    /** Unit of [com.forge.app.data.db.entities.CoachGoal.targetValue], for labels. */
    val unit: String,
    /** What [com.forge.app.data.db.entities.CoachGoal.targetKey] scopes this goal to. */
    val scope: Scope
) {
    /** Get stronger on one lift, measured by best working e1RM. */
    LIFT_1RM("lift_1rm", "Strength on a lift", "lb", Scope.EXERCISE),

    /** Build a muscle, measured by weekly working sets landing on it. */
    MUSCLE_VOLUME("muscle_volume", "Build a muscle", "sets/week", Scope.MUSCLE),

    /** Reach a bodyweight — up or down; the phase (cut/bulk) is read from the trend. */
    BODYWEIGHT("bodyweight", "Bodyweight target", "lb", Scope.NONE),

    /** Show up N times a week, measured over a 4-week window so one bad week isn't a verdict. */
    CONSISTENCY("consistency", "Train N times a week", "sessions/week", Scope.NONE),

    /** Weekly cardio minutes — the Engine plan's Health Floor / Base Build, in portfolio form. */
    CONDITIONING("conditioning", "Weekly conditioning", "min/week", Scope.NONE),

    /** Even out a push/pull or quad/ham set-volume ratio. */
    BALANCE("balance", "Fix an imbalance", "ratio", Scope.BALANCE_PAIR);

    enum class Scope { NONE, EXERCISE, MUSCLE, BALANCE_PAIR }

    companion object {
        fun fromCode(code: String): CoachGoalKind? = entries.firstOrNull { it.code == code }
    }
}
