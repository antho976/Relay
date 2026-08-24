package com.forge.app.domain.academy

/**
 * Lesson content (Coach v3 A2). One object per lesson, written in the app's voice: dry, specific,
 * imperative + "you", no exclamation marks, no hype, nothing that isn't grounded in what the coach
 * actually does.
 *
 * A lesson is 1–3 minutes. If it needs more than that, the coach concept behind it is too big.
 * Plain language is part of the contract: short sentences, everyday words, no term used before it
 * is defined.
 */
internal object AcademyContent {

    /** C1 — the readiness score, tapped through from the number itself. */
    val readinessBuiltFrom = Lesson(
        id = "coach.readiness_built_from",
        track = LessonTrack.COACH,
        title = "What your score is built from",
        summary = "Six readings, each one measured against your own normal rather than an average.",
        unlock = LessonUnlock(
            label = "Tap your readiness score",
            detail = "It sits on the Coach page once a score exists.",
            byYou = true
        ),
        blocks = listOf(
            LessonBlock.Paragraph(
                "Readiness is not a mystery number. It is a short list of readings, each worth a " +
                    "point or two, added up. The total is capped, so it can only ever nudge today's " +
                    "targets."
            ),
            LessonBlock.Bullets(
                listOf(
                    "Your morning check-in: sleep, soreness, stress and drive",
                    "Last night's measured sleep, when a watch reports it",
                    "Your resting heart rate against YOUR baseline, never an absolute number",
                    "How your last session felt, in your own words",
                    "Recent training and cardio, which draw on the same recovery",
                    "Anything you flagged: illness, injury, time away"
                )
            ),
            LessonBlock.Paragraph(
                "Your own answers count for a lot here. In the research they hold up at least as " +
                    "well as the hardware does. You usually know before your watch does."
            ),
            LessonBlock.Example(
                key = "readiness_parts",
                label = "Today",
                fallback = "your first readings will show here once a few sessions are logged"
            ),
            LessonBlock.Callout("Capped on purpose. Readiness shapes the session, it never cancels it.")
        )
    )

    /** C2 — fires the first time two of your goals genuinely fight. */
    val whyGoalsFight = Lesson(
        id = "coach.why_goals_fight",
        track = LessonTrack.COACH,
        title = "Why some goals fight each other",
        summary = "Two goals paid for out of the same recovery get put in an order rather than blended together.",
        unlock = LessonUnlock(
            label = "When two of your goals compete",
            detail = "Set more than one, and the coach flags it when they draw on the same recovery.",
            byYou = false
        ),
        blocks = listOf(
            LessonBlock.Paragraph(
                "Some goals sit together fine. A bench target, a consistency habit and an easy " +
                    "cardio base can all run at once, because none of them is paying for the others."
            ),
            LessonBlock.Paragraph(
                "Others do not. Losing weight means eating less than you burn. Adding to your best " +
                    "lift means recovering from heavy work. Both come out of the same energy and " +
                    "recovery, so chasing them together usually means doing neither well."
            ),
            LessonBlock.Heading("What the coach does instead"),
            LessonBlock.Bullets(
                listOf(
                    "Flags the pair, rather than quietly letting both go badly",
                    "Suggests an order, usually the shorter or more urgent goal first",
                    "Holds the other goal at a maintenance level instead of dropping it",
                    "Judges the strength goal more generously while you are eating less"
                )
            ),
            LessonBlock.Callout("Taking them in order is not giving one up. It is refusing to half-do both.")
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
        summary = "While you're losing weight, a lift that stays flat is a good result. That's why the coach stops calling it a plateau.",
        unlock = LessonUnlock(
            label = "When a lift holds while you're losing weight",
            detail = "The coach explains why it refuses to call that a plateau.",
            byYou = false
        ),
        blocks = listOf(
            LessonBlock.Paragraph(
                "When you're losing weight, losing a bit of strength with it is the normal " +
                    "outcome. You have less fuel to train on and less to recover with, so the " +
                    "honest expectation is that your lifts slip."
            ),
            LessonBlock.Paragraph(
                "So a lift that holds flat while your weight drops is not a stall. You kept the " +
                    "muscle behind it, and you're moving the same weight with less of you doing " +
                    "the moving. Pound for pound, you got stronger."
            ),
            LessonBlock.Heading("What the coach does about it"),
            LessonBlock.Bullets(
                listOf(
                    "Reads your weigh-ins as a trend, not as one morning's number",
                    "While that trend is down, it stops turning flat lifts into resets and swaps",
                    "It still speaks up if a lift actually drops, because holding is fine and sliding is not",
                    "When the trend flattens or turns up, the normal rules come straight back"
                )
            ),
            LessonBlock.Example(
                key = "weight_trend_per_week",
                label = "Your trend",
                fallback = "logging a few more weigh-ins will show your trend here"
            ),
            LessonBlock.Callout(
                "While you're cutting, judge a lift by what it holds, not by what it adds."
            ),
            LessonBlock.Paragraph(
                "The reverse is also true. If you're gaining weight and a lift stays flat for " +
                    "weeks, that is a real stall, and the coach treats it as one."
            )
        )
    )
}
