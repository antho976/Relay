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
        summary = "Better recovery between sets, between sessions, and more capacity for hard blocks.",
        unlock = LessonUnlock(
            label = "When the coach first prescribes cardio",
            detail = "Conditioning enters the plan once there's a lifting week to build it around.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Between your sets, your body clears the by-products of the last one and rebuilds " +
                    "the fuel for the next. That process is aerobic, and how good you are at it " +
                    "decides how much of your third and fourth set survives."
            ),
            Bullets(
                listOf(
                    "Faster recovery between sets, so late sets hold up",
                    "Faster recovery between sessions, which is what lets volume rise",
                    "A lower resting heart rate, which is a genuine health outcome on its own",
                    "More capacity for the high-volume phases of a block"
                )
            ),
            Paragraph(
                "None of that requires becoming a runner. The dose here is deliberately small: " +
                    "conditioning that serves your lifting, with a ceiling as well as a floor."
            ),
            Callout("Conditioning is a support system for training. It should never colonise your week.")
        )
    )

    val whatZone2Is = Lesson(
        id = "engine.what_zone2_is",
        track = LessonTrack.ENGINE,
        title = "What zone 2 actually is",
        summary = "The hardest you can go while still speaking in full sentences. That's the whole test.",
        unlock = LessonUnlock(
            label = "When your first zone-2 session is prescribed",
            detail = "The easy end of the range, where you can still talk in full sentences.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Zone 2 is easy aerobic work: the intensity where you could keep going for a long " +
                    "time, and where you can still talk in full sentences without gasping between " +
                    "them. That talk test is validated well enough to prescribe by, which matters " +
                    "because most people have no strap on."
            ),
            Paragraph(
                "It usually feels too easy, and that is the point. The adaptation it drives happens " +
                    "without meaningful recovery cost, so it can be added to a training week without " +
                    "taking anything from the training."
            ),
            Heading("If you do have heart rate"),
            Paragraph(
                "The coach anchors zones to YOUR max and resting heart rate rather than a poster on " +
                    "the gym wall. Without an age or a max entered, it makes no zone claims at all " +
                    "and prescribes by effort instead."
            ),
            Callout("Explained, not enforced. If you would rather do it by feel, the talk test is enough.")
        )
    )

    val interference = Lesson(
        id = "engine.interference",
        track = LessonTrack.ENGINE,
        title = "Interference: why placement matters",
        summary = "Hard cardio near hard lifting competes for the same recovery. Managed well, the cost is small.",
        unlock = LessonUnlock(
            label = "When cardio and lifting start competing",
            detail = "The coach either moves a session, or takes the cost out of your readiness.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The classic finding is that combining hard endurance work with strength training " +
                    "blunts some strength and power gains. The important detail, often lost, is " +
                    "where the effect concentrates: explosive qualities and long-duration running."
            ),
            Paragraph(
                "Modern reading is more forgiving. Managed for modality, dose and spacing, the cost " +
                    "is small, and that management is exactly what the Engine does for you."
            ),
            Bullets(
                listOf(
                    "Easy work costs almost nothing and can go nearly anywhere",
                    "Hard intervals get a day's clearance from heavy lower-body work",
                    "Cycling interferes less with squatting than running does",
                    "Yesterday's cardio load is subtracted from today's readiness, once"
                )
            ),
            Callout("Placement is most of the answer. You rarely have to choose between the two.")
        )
    )

    val readingHr = Lesson(
        id = "engine.reading_hr",
        track = LessonTrack.ENGINE,
        title = "Reading heart rate: zones, drift, resting",
        summary = "Zones are yours, not a poster's. Drift and resting HR are the two cheapest signals you have.",
        unlock = LessonUnlock(
            label = "Record a cardio session with heart rate",
            detail = "Needs a watch or strap feeding Health Connect while you train.",
            byYou = true
        ),
        blocks = listOf(
            Heading("Zones"),
            Paragraph(
                "Anchored to your own max and resting heart rate. A percentage-of-max chart ignores " +
                    "that a fitter heart sits lower at rest, which shifts every band."
            ),
            Heading("Drift"),
            Paragraph(
                "Cardiovascular drift is your heart rate climbing while your pace stays the same. " +
                    "It usually means heat, dehydration or simply duration, and it is why the coach " +
                    "may call a session before the timer does."
            ),
            Heading("Resting heart rate"),
            Paragraph(
                "The cheapest recovery signal there is: your own trend against your own baseline. " +
                    "A few beats up across a fortnight is worth noticing. One high morning is not."
            ),
            Callout("All three are trends about you. None of them mean anything as a single number.")
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
                "Intervals are the most time-efficient way to raise your ceiling, and the easiest " +
                    "way to wreck a training week. They cost real recovery, and that recovery comes " +
                    "out of the same account your lifting draws on."
            ),
            Bullets(
                listOf(
                    "At most one hard session a week, on top of an existing easy base",
                    "Never the day before heavy lower-body work",
                    "Always with a warm-up and a cool-down, which are part of the prescription",
                    "Dropped entirely during a deload week"
                )
            ),
            Callout("A small dose on a solid base. Intervals are never the default answer.")
        )
    )

    val baseWithoutALab = Lesson(
        id = "engine.base_without_a_lab",
        track = LessonTrack.ENGINE,
        title = "How your base is measured without a lab",
        summary = "Pace at the same effort, heart-rate drift, and resting heart rate, triangulated.",
        unlock = LessonUnlock(
            label = "When your aerobic base shows a trend",
            detail = "Needs enough sessions to compare pace at the same heart rate.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "You do not need a lactate test to know whether your base is improving. Three cheap " +
                    "signals, read together, are enough to steer by."
            ),
            Bullets(
                listOf(
                    "Same route and same effort, covered faster, means fitter",
                    "Less heart-rate drift within a steady session means better endurance",
                    "A falling resting heart rate over weeks points the same way"
                )
            ),
            Paragraph(
                "Sessions logged in heat, cold, rain or wind are left out of the pace comparison. " +
                    "Weather inflates heart rate and slows pace, and counting those would read as " +
                    "lost fitness that never happened."
            ),
            Callout("It's your own trend against your own history. That's the only fair comparison.")
        )
    )

    val all: List<Lesson> = listOf(
        whyAerobicBase, whatZone2Is, interference, readingHr, intervals, baseWithoutALab
    )
}
