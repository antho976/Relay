package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The Engine track (E1–E6) — conditioning explained for a lifter, not for a runner.
 *
 * The anti-dogma rule from the Engine plan holds throughout: zone 2 is explained, never enforced,
 * and every lesson says what the evidence actually supports.
 */
internal object AcademyEngine {

    val whyAerobicBase = Lesson(
        id = "engine.why_aerobic_base",
        track = LessonTrack.ENGINE,
        title = "Why lifters need an aerobic base",
        summary = "You recover faster between sets, faster between sessions, and handle more work in a hard block.",
        unlock = LessonUnlock(
            label = "When the coach first prescribes cardio",
            detail = "Conditioning enters the plan once there's a lifting week to build it around.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Between sets, your body clears out what the last one left behind and rebuilds the " +
                    "fuel for the next. That job runs on your aerobic system, and how good yours is " +
                    "decides how much of your third and fourth set is left."
            ),
            Bullets(
                listOf(
                    "You recover faster between sets, so your later sets hold up",
                    "You recover faster between sessions, which is what lets your volume rise",
                    "Your resting heart rate drops, which is worth having on its own",
                    "You handle the high-volume stretch of a block better"
                )
            ),
            Paragraph(
                "None of this means becoming a runner. The dose is deliberately small, and it has " +
                    "a ceiling as well as a floor."
            ),
            Callout("Conditioning is there to support your lifting. It should never take over your week.")
        )
    )

    val whatZone2Is = Lesson(
        id = "engine.what_zone2_is",
        track = LessonTrack.ENGINE,
        title = "What zone 2 actually is",
        summary = "The hardest you can go while still talking in full sentences. That's the whole test.",
        unlock = LessonUnlock(
            label = "When your first zone-2 session is prescribed",
            detail = "The easy end of the range, where you can still talk in full sentences.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Zone 2 is easy aerobic work. It's the pace you could hold for a long time, where " +
                    "you can still talk in full sentences without gasping between them. That talk " +
                    "test is accurate enough to train by, which matters, because most people " +
                    "aren't wearing a heart-rate strap."
            ),
            Paragraph(
                "It usually feels too easy, and that's the point. It builds what it builds without " +
                    "costing you much recovery, so it fits into a training week without stealing " +
                    "from the lifting."
            ),
            Heading("If you do have heart rate"),
            Paragraph(
                "The coach builds your zones from YOUR max and resting heart rate, not from the " +
                    "chart on the gym wall. With neither your age nor a max, it doesn't guess at " +
                    "zones at all. It gives you the session by effort instead."
            ),
            Callout("Explained, not enforced. If you would rather do it by feel, the talk test is enough.")
        )
    )

    val interference = Lesson(
        id = "engine.interference",
        track = LessonTrack.ENGINE,
        title = "Interference: why placement matters",
        summary = "Hard cardio next to hard lifting draws on the same recovery. Spaced well, the cost is small.",
        unlock = LessonUnlock(
            label = "When cardio and lifting start competing",
            detail = "The coach either moves a session, or takes the cost out of your readiness.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The old finding is that hard endurance work alongside lifting eats into your " +
                    "strength and power gains. The detail that usually gets dropped is where it " +
                    "actually bites: explosive work, and long runs."
            ),
            Paragraph(
                "The newer read is more forgiving. Pick the right kind of cardio, keep the dose " +
                    "sensible, and leave space between the hard sessions, and the cost is small. " +
                    "That spacing is the work the coach is doing for you here."
            ),
            Bullets(
                listOf(
                    "Easy work costs almost nothing and can go almost anywhere in the week",
                    "Hard intervals get a day's clearance from heavy leg work",
                    "Cycling gets in the way of squatting less than running does",
                    "Yesterday's cardio comes out of today's readiness, once"
                )
            ),
            Callout("Where you put it is most of the answer. You rarely have to pick one or the other.")
        )
    )

    val readingHr = Lesson(
        id = "engine.reading_hr",
        track = LessonTrack.ENGINE,
        title = "Reading heart rate: zones, drift, resting",
        summary = "Your zones are yours, not a poster's. Drift and resting heart rate are the two cheapest signals you have.",
        unlock = LessonUnlock(
            label = "Record a cardio session with heart rate",
            detail = "Needs a watch or strap feeding Health Connect while you train.",
            byYou = true
        ),
        blocks = listOf(
            Heading("Zones"),
            Paragraph(
                "Built from your own max and resting heart rate. A plain percentage-of-max chart " +
                    "ignores the fact that a fitter heart sits lower at rest, and that moves every " +
                    "zone."
            ),
            Heading("Drift"),
            Paragraph(
                "Drift is your heart rate climbing while your pace stays the same. Usually it " +
                    "means heat, not enough fluid, or simply time on your feet. It's why the coach " +
                    "may end a session before the timer does."
            ),
            Heading("Resting heart rate"),
            Paragraph(
                "The cheapest recovery signal you have: your own trend against your own baseline. " +
                    "A few beats up across two weeks is worth noticing. One high morning is not."
            ),
            Callout("All three are trends. None of them mean much as a single reading.")
        )
    )

    val intervals = Lesson(
        id = "engine.intervals",
        track = LessonTrack.ENGINE,
        title = "Intervals: dose and recovery",
        summary = "They buy top-end fitness at a steep recovery price, which is why they're rationed.",
        unlock = LessonUnlock(
            label = "When intervals are first prescribed",
            detail = "Rationed on top of your base, never the default answer.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Intervals are the fastest way to raise your ceiling and the easiest way to wreck " +
                    "a training week. They cost real recovery, and it comes out of the same pot " +
                    "your lifting draws on."
            ),
            Bullets(
                listOf(
                    "One hard session a week at most, on top of an easy base you already have",
                    "Never the day before heavy leg work",
                    "Always with a warm-up and a cool-down, which are part of the session",
                    "Dropped completely during a deload week"
                )
            ),
            Callout("A small dose on a solid base. Intervals are never the default answer.")
        )
    )

    val baseWithoutALab = Lesson(
        id = "engine.base_without_a_lab",
        track = LessonTrack.ENGINE,
        title = "How your base is measured without a lab",
        summary = "Three cheap signals read together: pace at the same effort, drift inside a session, and resting heart rate.",
        unlock = LessonUnlock(
            label = "When your aerobic base shows a trend",
            detail = "Needs enough sessions to compare pace at the same heart rate.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "You don't need a lab test to know whether your base is improving. Three cheap " +
                    "signals, read together, are enough to steer by."
            ),
            Bullets(
                listOf(
                    "Same route and same effort, covered faster, means fitter",
                    "Less heart-rate climb inside a steady session means better endurance",
                    "A resting heart rate falling over weeks points the same way"
                )
            ),
            Paragraph(
                "Sessions in heat, cold, rain or wind are left out of the pace comparison. Weather " +
                    "pushes your heart rate up and your pace down, and counting those days would " +
                    "look like fitness you never lost."
            ),
            Callout("It's your own trend against your own history. That's the only fair comparison.")
        )
    )

    val all: List<Lesson> = listOf(
        whyAerobicBase, whatZone2Is, interference, readingHr, intervals, baseWithoutALab
    )
}
