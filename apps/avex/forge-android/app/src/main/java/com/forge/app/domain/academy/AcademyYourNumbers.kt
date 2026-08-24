package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Example
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The "your numbers" batch (Coach v3 D) — P5–P8 plus C4.
 *
 * These are the lessons that only make sense once the coach has measured YOU: your volume
 * landmarks, your recovery spacing, your best rep ranges, and what it does with them. Each one
 * shows the population default beside the personal number, because the comparison is the lesson.
 */
internal object AcademyYourNumbers {

    val volumeLandmarks = Lesson(
        id = "programming.your_volume_landmarks",
        track = LessonTrack.PROGRAMMING,
        title = "Your volume floor and ceiling",
        summary = "The least work that still moves you forward, and the most you can recover from. Train between the two.",
        unlock = LessonUnlock(
            label = "When it measures your own volume ceiling",
            detail = "It starts from population defaults and replaces them as your log shows what you recover from.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Two numbers bracket useful training for any muscle. The floor is the least weekly " +
                    "work that still makes you better. The ceiling is the most you can recover " +
                    "from before extra work stops paying you back."
            ),
            Paragraph(
                "More sets help, up to a point. Past your own ceiling, extra sets buy fatigue " +
                    "instead of growth."
            ),
            Heading("Where your numbers came from"),
            Paragraph(
                "They started as averages. Since then the coach has watched your own weeks and " +
                    "asked one question: when a muscle got more work, did you come out stronger? " +
                    "If yes, your ceiling moved up. If not, it came down."
            ),
            Example(
                key = "volume_caps",
                label = "Your caps",
                fallback = "a couple more months of history and your own numbers appear here"
            ),
            Paragraph(
                "Both numbers are estimates read off your training, not measurements. Treat them " +
                    "as a sensible range to work inside, not a line you must not cross."
            ),
            Callout("Your ceiling can only drift so far from the average. One odd month shouldn't rewrite your training.")
        )
    )

    val recoveryCurve = Lesson(
        id = "programming.your_recovery_curve",
        track = LessonTrack.PROGRAMMING,
        title = "Your recovery curve",
        summary = "How many days you personally need before a muscle performs again.",
        unlock = LessonUnlock(
            label = "When your recovery pace changes the schedule",
            detail = "Estimated from how you perform bout to bout on the same muscle.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The usual advice is to train a muscle every two to three days. It's a range " +
                    "because the real answer moves with how heavy you go, how much you do, your " +
                    "age, your sleep, and how physical the rest of your life already is."
            ),
            Paragraph(
                "The coach reads yours off your own sessions. At what gap does your work actually " +
                    "hold up? That's the spacing it plans around."
            ),
            Example(
                key = "recovery_days",
                label = "Your spacing",
                fallback = "a few more months of sessions and this will read your own gap"
            ),
            Callout("Frequency is a lever, not a rule. How much you do in a week matters more than how you split it.")
        )
    )

    val sweetSpotReps = Lesson(
        id = "programming.sweet_spot_reps",
        track = LessonTrack.PROGRAMMING,
        title = "Your sweet-spot rep ranges",
        summary = "Muscle grows across a wide rep range. Inside it, your log shows where you actually progress.",
        unlock = LessonUnlock(
            label = "When your best rep range emerges",
            detail = "Your log has to show where you actually progress before prescriptions lean there.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Muscle grows across a wide span of reps, roughly five to thirty, as long as the " +
                    "sets get close enough to failure. Strength is fussier. It follows the weights " +
                    "you actually train with."
            ),
            Paragraph(
                "That leaves room for your own data to decide. The coach sorts your sessions by " +
                    "rep count, checks which group carried the most progress, and leans your " +
                    "targets that way."
            ),
            Callout("The best rep range is the one you progress and recover on. That's measurable, and it's yours.")
        )
    )

    val imbalances = Lesson(
        id = "programming.imbalances",
        track = LessonTrack.PROGRAMMING,
        title = "Imbalances, and why the coach hunts them",
        summary = "The gap between opposing muscle groups, measured in your own sets and your own strength.",
        unlock = LessonUnlock(
            label = "When it finds a gap between opposing groups",
            detail = "It watches push against pull, and quads against hamstrings, in both volume and strength.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "An imbalance is a gap that keeps showing up between opposing groups: pushing " +
                    "against pulling, quads against hamstrings. The coach measures it in the work " +
                    "you actually log, not in how you look."
            ),
            Heading("Why it's worth fixing"),
            Bullets(
                listOf(
                    "The weaker side eventually caps the stronger one, so the plateau turns up anyway",
                    "The joint-health argument is real, but the evidence behind it is thinner than the evidence on volume",
                    "It's usually the cheapest fix available: a couple of sets a week, not a new program"
                )
            ),
            Callout("A catch-up block is boring and it works. That is the whole method.")
        )
    )

    val whatAProjectIs = Lesson(
        id = "coach.what_a_project_is",
        track = LessonTrack.COACH,
        title = "What a project is",
        summary = "One named improvement at a time, with a reason, a plan and a finish line.",
        unlock = LessonUnlock(
            label = "When the coach proposes a project",
            detail = "It hunts for your biggest single lever and runs one at a time.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The coach is always looking for the single biggest thing you could improve: a " +
                    "lagging muscle, an imbalance, no conditioning, short sleep, work you keep " +
                    "skipping. It proposes one at a time."
            ),
            Paragraph(
                "One is deliberate. A list of eight things to fix is a list nobody acts on. One " +
                    "project with a stated finish line is a thing that gets finished."
            ),
            Bullets(
                listOf(
                    "Every project says why it exists, in your own numbers",
                    "Every project says what ends it",
                    "You can drop one at any time, and the coach won't propose that kind again",
                    "Finished and dropped projects both stay on the record"
                )
            ),
            Callout("This is \"what should I improve?\" answered before you have to ask it.")
        )
    )

    val all: List<Lesson> = listOf(
        volumeLandmarks, recoveryCurve, sweetSpotReps, imbalances, whatAProjectIs
    )
}
