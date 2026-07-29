package com.forge.app.domain.academy

import com.forge.app.domain.academy.LessonBlock.Bullets
import com.forge.app.domain.academy.LessonBlock.Callout
import com.forge.app.domain.academy.LessonBlock.Example
import com.forge.app.domain.academy.LessonBlock.Heading
import com.forge.app.domain.academy.LessonBlock.Paragraph

/**
 * The Fundamentals track (Coach v3 B3) — F1–F10, the only sequential track, and the one that makes
 * Decision Zero true for a day-one user: below the data gates these lessons ARE the directive.
 *
 * Written to the plan's bound: teach exactly what the coach does, no more. Every lesson here
 * explains something the app will actually do to you, so a user who reads all ten could override
 * any of it. Sources for each are in `docs/ACADEMY_LESSONS.md`; lessons themselves cite nothing.
 */
internal object AcademyFundamentals {

    val whatAProgramIs = Lesson(
        id = "fundamentals.what_a_program_is",
        track = LessonTrack.FUNDAMENTALS,
        title = "What a program is",
        summary = "A repeatable structure is what makes progress measurable. Change everything every week and nothing can be judged.",
        unlock = LessonUnlock(
            label = "Open your first program day",
            detail = "Unlocks as soon as there's a plan to open.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "A program is a repeatable structure: a set of days, each with a set of exercise " +
                    "slots. The point is not the specific movements. The point is that they repeat, " +
                    "so the numbers you log this week can be compared to last week's."
            ),
            Paragraph(
                "If the session changes completely every time, nothing can be measured, and nothing " +
                    "can be improved on purpose. You would be exercising rather than training."
            ),
            Heading("Why this one"),
            Bullets(
                listOf(
                    "It fits the days a week you said you can train",
                    "It only uses equipment you said you have",
                    "It spreads work across muscles so nothing gets trained into the ground",
                    "You can change any of it, and the coach adapts to what you actually do"
                )
            ),
            Callout("Repeatable beats optimal. A structure you follow beats a better one you don't.")
        )
    )

    val setsRepsRpe = Lesson(
        id = "fundamentals.sets_reps_rpe",
        track = LessonTrack.FUNDAMENTALS,
        title = "Sets, reps, and how hard is hard",
        summary = "Reps are what you do, RPE is how close to your limit you did it. The coach needs both.",
        unlock = LessonUnlock(
            label = "Log a set",
            detail = "Start any session and log one working set.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "A prescription like 3 by 8 to 10 means three working sets, each landing somewhere " +
                    "between eight and ten reps. The range exists so you can add reps before you add " +
                    "weight."
            ),
            Heading("Effort, in one number"),
            Paragraph(
                "RPE rates how hard a set was on a 1 to 10 scale, judged by what you had left. " +
                    "RPE 8 means you could have done two more clean reps. RPE 10 means there was " +
                    "nothing left."
            ),
            Bullets(
                listOf(
                    "RPE 6 to 7: comfortable, several reps left",
                    "RPE 8: two clean reps left, and the usual target for working sets",
                    "RPE 9: one rep left",
                    "RPE 10: you could not have done another"
                )
            ),
            Paragraph(
                "Most training should sit around 7 to 9. Effort is the dial, not exhaustion: " +
                    "training to your absolute limit every set costs more recovery than it buys."
            ),
            Callout("Log effort honestly and the coach can tell a hard day from a heavy one.")
        )
    )

    val formVsLoad = Lesson(
        id = "fundamentals.form_vs_load",
        track = LessonTrack.FUNDAMENTALS,
        title = "Form first, load second",
        summary = "Load only counts for the muscle actually doing the work.",
        unlock = LessonUnlock(
            label = "Tag a session as technique work",
            detail = "Finish a session and add the technique tag. The coach then leaves those sets out of stall detection.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "Weight on the bar is a means, not the goal. A heavier lift performed with a shorter " +
                    "range of motion or with other muscles taking over is not more stimulus for the " +
                    "muscle you were training, it is just a bigger number."
            ),
            Bullets(
                listOf(
                    "Control the lowering phase rather than dropping into it",
                    "Use the full range you can control, not the range that lets you add plates",
                    "Keep the working muscle working, instead of bouncing or swinging the weight",
                    "If your form changes to make a rep, that rep is past your limit"
                )
            ),
            Paragraph(
                "This is why you can tag a session as technique work. Tagged sessions are kept out " +
                    "of the coach's progress reads, so a deliberately light day never reads as a stall."
            ),
            Callout("The coach never rewards an ugly personal record, because your body doesn't either.")
        )
    )

    val progressiveOverload = Lesson(
        id = "fundamentals.progressive_overload",
        track = LessonTrack.FUNDAMENTALS,
        title = "Progressive overload",
        summary = "Doing slightly more over time is the signal that forces adaptation. Everything else is detail.",
        unlock = LessonUnlock(
            label = "When the coach first suggests a weight",
            detail = "It needs two sessions of the same lift to compare before it will.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Your body adapts to demands it keeps meeting. If the demand never rises, there is " +
                    "nothing to adapt to, and you hold where you are."
            ),
            Heading("More can mean several things"),
            Bullets(
                listOf(
                    "More weight for the same reps",
                    "More reps at the same weight",
                    "More sets across the week",
                    "Better control or range at the same numbers"
                )
            ),
            Paragraph(
                "The coach uses double progression: fill the rep range first, then add weight and " +
                    "start filling it again. That way the jump is earned rather than guessed."
            ),
            Callout("Your job is showing up and logging honestly. Choosing which \"more\" is the coach's job.")
        )
    )

    val restAndRecovery = Lesson(
        id = "fundamentals.rest_and_recovery",
        track = LessonTrack.FUNDAMENTALS,
        title = "Rest is where you grow",
        summary = "Training is the stimulus. The adaptation happens between sessions.",
        unlock = LessonUnlock(
            label = "Use a rest timer",
            detail = "It starts itself the moment you log a set in a session.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "Two clocks matter. The one between sets, and the one between sessions."
            ),
            Heading("Between sets"),
            Paragraph(
                "On heavy compound lifts, two to three minutes lets you repeat a real effort. " +
                    "Cutting rest to one minute mostly means your later sets are worse, which is not " +
                    "the same as training harder."
            ),
            Heading("Between sessions"),
            Paragraph(
                "Muscle rebuilds over roughly two to three days after you train it. That is why the " +
                    "coach spaces the same muscle out, and why a rest day is a scheduled part of the " +
                    "plan and not a failure to train."
            ),
            Callout("Sleep is the highest-value recovery tool you own, and the cheapest.")
        )
    )

    val sorenessVsInjury = Lesson(
        id = "fundamentals.soreness_vs_injury",
        track = LessonTrack.FUNDAMENTALS,
        title = "Soreness vs injury",
        summary = "Soreness is dull, spread out and fades with a warm-up. Injury is sharp, local and worse under load.",
        unlock = LessonUnlock(
            label = "Flag soreness or illness",
            detail = "From the daily check-in, or by logging a rest day as sick.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "Delayed soreness peaks a day or two after training, feels dull and spread across " +
                    "the muscle, affects both sides evenly, and usually eases once you warm up."
            ),
            Paragraph(
                "An injury feels different. It is sharp rather than dull, sits in one spot, often " +
                    "near a joint, and gets worse as you load it rather than better."
            ),
            Bullets(
                listOf(
                    "Sore: train it lighter, or train something else, and it settles",
                    "Sharp or joint-centred: stop loading it and flag it as restricted",
                    "Soreness is not proof of a good session, and its absence is not proof of a bad one",
                    "You get less sore over time on the same movements, even while still progressing"
                )
            ),
            Paragraph(
                "When you flag a muscle as sore, the coach eases that muscle's work for a day or two. " +
                    "When you flag an injury, it routes around the movement entirely until you clear it."
            ),
            Callout("Soreness is information. It is not a score.")
        )
    )

    val warmups = Lesson(
        id = "fundamentals.warmups",
        track = LessonTrack.FUNDAMENTALS,
        title = "Why warm-ups are in your session",
        summary = "A warm-up raises tissue temperature and rehearses the movement. It should never tire you out.",
        unlock = LessonUnlock(
            label = "Start a session with its warm-up",
            detail = "Warm-up sets lead a session unless you hold the start button to skip them.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "Warming up makes tissue more pliable and gives your nervous system a rehearsal of " +
                    "the pattern you are about to load. The measurable payoff is performance: your " +
                    "first working set is closer to your real capacity."
            ),
            Heading("Ramp sets"),
            Paragraph(
                "The simplest warm-up for a lift is the lift itself, ascending: a few light reps, " +
                    "then a heavier set of a few, then your working weight. Low reps, never near " +
                    "failure. The aim is to arrive fresh, not to pre-fatigue yourself."
            ),
            Callout("If your warm-up made you tired, it was training, and it will cost you.")
        )
    )

    val howTheCoachWorks = Lesson(
        id = "fundamentals.how_the_coach_works",
        track = LessonTrack.FUNDAMENTALS,
        title = "How your coach works",
        summary = "Snapshot your data, propose a change, watch what happens, and keep score of itself.",
        unlock = LessonUnlock(
            label = "When your first week brief lands",
            detail = "The coach writes one after a week with training in it.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "The coach runs one loop. It reads a snapshot of everything you have logged, its " +
                    "advisors propose changes, you approve or ignore them, and a watcher judges every " +
                    "applied change once enough time has passed to tell."
            ),
            Bullets(
                listOf(
                    "Nothing is written without being watched afterwards",
                    "Everything it changes can be undone in one tap",
                    "Changes that work earn it the right to make bigger ones",
                    "Changes that fail get folded back into how it plans",
                    "Below its data gates it says nothing, rather than guessing"
                )
            ),
            Paragraph(
                "That last rule is why a new account sees fewer suggestions. Sparse data produces " +
                    "confident nonsense, so the coach waits until it has enough to be useful."
            ),
            Callout("It is a system you can inspect, not an oracle. Every call shows its reason.")
        )
    )

    val whatReadinessMeans = Lesson(
        id = "fundamentals.what_readiness_means",
        track = LessonTrack.FUNDAMENTALS,
        title = "What readiness means",
        summary = "A small nudge to today's targets, built from how you slept, felt and trained.",
        unlock = LessonUnlock(
            label = "When a readiness score first appears",
            detail = "Needs recovery data: a check-in answer, or sleep and resting heart rate from a watch.",
            byYou = false
        ),
        blocks = listOf(
            Paragraph(
                "Readiness answers one question: how much should today ask of you? It is built from " +
                    "your check-in, your sleep, your resting heart rate against your own baseline, " +
                    "recent training load, and anything you flagged."
            ),
            Paragraph(
                "It moves your targets by a few percent at most. It never cancels your session and " +
                    "it never tells you that you are broken. A low score is information about today, " +
                    "not a verdict about you."
            ),
            Example(
                key = "readiness_today",
                label = "Today's read",
                fallback = "log a few more sessions and today's number will appear here"
            ),
            Callout("A bad night is a reason to train slightly lighter, not a reason to skip.")
        )
    )

    val logHonestly = Lesson(
        id = "fundamentals.log_honestly",
        track = LessonTrack.FUNDAMENTALS,
        title = "Log honestly",
        summary = "Every decision upstream is a function of your log. Inflated numbers don't cheat the coach, they mis-aim it.",
        unlock = LessonUnlock(
            label = "Finish the fundamentals track",
            detail = "Read the nine lessons above. This one closes it.",
            byYou = true
        ),
        blocks = listOf(
            Paragraph(
                "The coach has no eyes. Everything it believes about you comes from what you logged: " +
                    "the weight, the reps, the effort, whether you finished the set."
            ),
            Bullets(
                listOf(
                    "A rep you did not really complete becomes a target you cannot hit",
                    "An RPE lower than the truth makes the next jump too big",
                    "A skipped soreness flag turns a manageable week into an unexplained stall",
                    "A session you did not log is a session the coach thinks you never did"
                )
            ),
            Paragraph(
                "Rating effort accurately is a skill, and it improves with practice. Early on you " +
                    "will guess. That is fine. Guess consistently and it becomes useful data fast."
            ),
            Callout("Honest logging is the entire price of a coach that can actually help.")
        )
    )

    /** The track in reading order — it doubles as the cold-start directive sequence. */
    val ordered: List<Lesson> = listOf(
        whatAProgramIs,
        setsRepsRpe,
        formVsLoad,
        progressiveOverload,
        restAndRecovery,
        sorenessVsInjury,
        warmups,
        howTheCoachWorks,
        whatReadinessMeans,
        logHonestly
    )
}
