package com.forge.app.domain.academy

/**
 * Lesson content (Coach v3 A2). One object per lesson, written in the app's voice: dry, specific,
 * imperative + "you", no exclamation marks, no hype, nothing that isn't grounded in what the coach
 * actually does.
 *
 * A lesson is 1–3 minutes. If it needs more than that, the coach concept behind it is too big.
 */
internal object AcademyContent {

    /** C1 — the readiness score, tapped through from the number itself. */
    val readinessBuiltFrom = Lesson(
        id = "coach.readiness_built_from",
        track = LessonTrack.COACH,
        title = "What your score is built from",
        summary = "Six readings, each one yours rather than a population average.",
        unlockedBy = "Tapping your readiness score",
        blocks = listOf(
            LessonBlock.Paragraph(
                "Readiness is not a mystery number. It is a short list of readings, each worth a " +
                    "point or two, added up and capped so it can only ever nudge today's targets."
            ),
            LessonBlock.Bullets(
                listOf(
                    "Your morning check-in: sleep, soreness, stress and drive",
                    "Last night's measured sleep, when a watch reports it",
                    "Resting heart rate against YOUR baseline, never an absolute number",
                    "How your last session felt, in your own words",
                    "Recent training and cardio load, which competes for the same recovery",
                    "Anything you flagged: illness, injury, time away"
                )
            ),
            LessonBlock.Paragraph(
                "Subjective answers are weighted seriously here, because in the research they hold " +
                    "up at least as well as the gadgets. You usually know before your watch does."
            ),
            LessonBlock.Example(
                key = "readiness_parts",
                label = "Today",
                fallback = "your first readings will show here once a few sessions are logged"
            ),
            LessonBlock.Callout("Bounded on purpose: readiness shapes the session, it never cancels it.")
        )
    )

    /** C2 — fires the first time two of your goals genuinely fight. */
    val whyGoalsFight = Lesson(
        id = "coach.why_goals_fight",
        track = LessonTrack.COACH,
        title = "Why some goals fight each other",
        summary = "Physiology has budgets. Two goals drawing on the same one get sequenced, not blended.",
        unlockedBy = "The first conflict flagged in your goals",
        blocks = listOf(
            LessonBlock.Paragraph(
                "Some goals are genuinely compatible. A bench target, a consistency habit and an " +
                    "easy cardio base can all run at once, because they draw on different budgets."
            ),
            LessonBlock.Paragraph(
                "Others do not. Losing weight means eating less than you burn, and adding to a " +
                    "maximum lift means recovering from heavy work. Both draw on the same energy and " +
                    "recovery budget, so pursuing them at once usually means doing neither well."
            ),
            LessonBlock.Heading("What the coach does instead"),
            LessonBlock.Bullets(
                listOf(
                    "Flags the pair rather than quietly degrading both",
                    "Proposes an order, usually the shorter or more time-sensitive goal first",
                    "Keeps the other goal on a maintenance floor rather than dropping it",
                    "Re-reads the strength goal generously while you are in a deficit"
                )
            ),
            LessonBlock.Callout("Sequencing is not giving up on one. It is refusing to half-do both.")
        )
    )

    /**
     * C3 — ships with A2 because A2 is when the coach starts behaving this way. The plan's audit
     * rule is that no release series ends with a shipped concept and no lesson.
     */
    val strengthOnACut = Lesson(
        id = "coach.strength_on_a_cut",
        track = LessonTrack.COACH,
        title = "Holding strength while cutting is winning",
        summary = "In a deficit, a flat lift is a good outcome. That's why the coach stops calling it a plateau.",
        unlockedBy = "The first time the coach holds its fire on a stalled lift because you're losing weight",
        blocks = listOf(
            LessonBlock.Paragraph(
                "When you're losing weight, the default outcome is losing strength with it. " +
                    "You have less fuel to train on and less to recover with, so the honest " +
                    "expectation for a lift in a deficit is that it slips."
            ),
            LessonBlock.Paragraph(
                "So a lift that holds flat while your weight drops is not a stall. You kept the " +
                    "muscle that lift depends on, and you're moving the same load at a lighter " +
                    "bodyweight, which means you got relatively stronger."
            ),
            LessonBlock.Heading("What the coach does about it"),
            LessonBlock.Bullets(
                listOf(
                    "Reads your weigh-ins for a trend, not a single morning number",
                    "While that trend is down, it stops escalating flat lifts into resets and swaps",
                    "It still speaks up if a lift actually declines, because holding is fine and sliding is not",
                    "When the trend flattens or turns up, normal progression rules come straight back"
                )
            ),
            LessonBlock.Example(
                key = "weight_trend_per_week",
                label = "Your trend",
                fallback = "logging a few more weigh-ins will show your trend here"
            ),
            LessonBlock.Callout(
                "In a deficit, judge a lift by what it holds, not by what it adds."
            ),
            LessonBlock.Paragraph(
                "The reverse is also true. If you're gaining weight and a lift stays flat for " +
                    "weeks, that is a real stall, and the coach treats it as one."
            )
        )
    )
}
